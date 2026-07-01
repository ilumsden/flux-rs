use std::{
    ffi::{c_char, c_void, CStr, CString},
    fmt::Display,
};

use bitflags::bitflags;
use flux_sys::core::{
    flux_match, flux_msg_authorize, flux_msg_clear_flag, flux_msg_cmp, flux_msg_cmp_matchtag,
    flux_msg_copy, flux_msg_create, flux_msg_cred, flux_msg_cred_authorize, flux_msg_decode,
    flux_msg_destroy, flux_msg_encode, flux_msg_encode_size, flux_msg_get_cred,
    flux_msg_get_errnum, flux_msg_get_matchtag, flux_msg_get_nodeid, flux_msg_get_payload,
    flux_msg_get_seq, flux_msg_get_string, flux_msg_get_topic, flux_msg_has_flag,
    flux_msg_has_payload, flux_msg_incref, flux_msg_is_local, flux_msg_is_noresponse,
    flux_msg_is_private, flux_msg_is_streaming, flux_msg_set_cred, flux_msg_set_errnum,
    flux_msg_set_flag, flux_msg_set_matchtag, flux_msg_set_nodeid, flux_msg_set_noresponse,
    flux_msg_set_payload, flux_msg_set_private, flux_msg_set_seq, flux_msg_set_streaming,
    flux_msg_set_string, flux_msg_set_topic, flux_msg_t, flux_msg_typestr, FLUX_MATCHTAG_NONE,
    FLUX_MSGFLAG_NORESPONSE, FLUX_MSGFLAG_PAYLOAD, FLUX_MSGFLAG_PRIVATE, FLUX_MSGFLAG_ROUTE,
    FLUX_MSGFLAG_STREAMING, FLUX_MSGFLAG_TOPIC, FLUX_MSGFLAG_UPSTREAM, FLUX_MSGFLAG_USER1,
    FLUX_MSGTYPE_ANY, FLUX_MSGTYPE_CONTROL, FLUX_MSGTYPE_EVENT, FLUX_MSGTYPE_MASK,
    FLUX_MSGTYPE_REQUEST, FLUX_MSGTYPE_RESPONSE, FLUX_ROLE_ALL, FLUX_ROLE_LOCAL, FLUX_ROLE_NONE,
    FLUX_ROLE_OWNER, FLUX_ROLE_USER,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{check_ptr, check_rc, FluxError, Result};

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MessageType: u32 {
        const NONE = 0;
        const REQUEST = FLUX_MSGTYPE_REQUEST;
        const RESPONSE = FLUX_MSGTYPE_RESPONSE;
        const EVENT = FLUX_MSGTYPE_EVENT;
        const CONTROL = FLUX_MSGTYPE_CONTROL;
        const ANY = FLUX_MSGTYPE_ANY;
        const MASK = FLUX_MSGTYPE_MASK;
    }
}

impl Display for MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let typestr_ptr = unsafe { flux_msg_typestr(self.bits() as i32) };
        if typestr_ptr.is_null() {
            return write!(f, "unknown");
        }
        let typestr = unsafe { CStr::from_ptr(typestr_ptr).to_string_lossy() };
        write!(f, "{}", typestr)
    }
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MessageFlag: u32 {
        const NONE = 0;
        const TOPIC = FLUX_MSGFLAG_TOPIC;
        const PAYLOAD = FLUX_MSGFLAG_PAYLOAD;
        const NORESPONSE = FLUX_MSGFLAG_NORESPONSE;
        const ROUTE = FLUX_MSGFLAG_ROUTE;
        const UPSTREAM = FLUX_MSGFLAG_UPSTREAM;
        const PRIVATE = FLUX_MSGFLAG_PRIVATE;
        const STREAMING = FLUX_MSGFLAG_STREAMING;
        const USER1 = FLUX_MSGFLAG_USER1;
    }
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MessageRolemask: u32 {
        const NONE = FLUX_ROLE_NONE;
        const OWNER = FLUX_ROLE_OWNER;
        const USER = FLUX_ROLE_USER;
        const LOCAL = FLUX_ROLE_LOCAL;
        const ALL = FLUX_ROLE_ALL;
    }
}

pub struct MessageMatch {
    typemask: Option<MessageType>,
    matchtag: Option<u32>,
    topic_glob: Option<CString>,
}

impl MessageMatch {
    pub fn new(
        typemask: Option<MessageType>,
        matchtag: Option<u32>,
        topic_glob: Option<&str>,
    ) -> Result<Self> {
        let c_topic_glob = topic_glob
            .map(|rust_str| CString::new(rust_str))
            .transpose()?;
        Ok(Self {
            typemask,
            matchtag,
            topic_glob: c_topic_glob,
        })
    }

    pub fn set_typemask(&mut self, typemask: Option<MessageType>) {
        self.typemask = typemask;
    }

    pub fn set_matchtag(&mut self, matchtag: Option<u32>) {
        self.matchtag = matchtag;
    }

    pub fn set_topic_glob(&mut self, topic_glob: Option<&str>) -> Result<()> {
        let c_topic_glob = topic_glob
            .map(|rust_str| CString::new(rust_str))
            .transpose()?;
        self.topic_glob = c_topic_glob;
        Ok(())
    }
}

impl TryFrom<flux_match> for MessageMatch {
    type Error = FluxError;

    fn try_from(value: flux_match) -> Result<Self> {
        let ru_topic_glob = if value.topic_glob.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(value.topic_glob).to_owned() })
        };
        Ok(Self {
            typemask: Some(MessageType::from_bits_retain(value.typemask as u32)),
            matchtag: Some(value.matchtag),
            topic_glob: ru_topic_glob,
        })
    }
}

impl From<&MessageMatch> for flux_match {
    fn from(value: &MessageMatch) -> Self {
        flux_match {
            typemask: value.typemask.map_or(0, |t| t.bits() as i32),
            matchtag: value.matchtag.unwrap_or(0),
            topic_glob: value
                .topic_glob
                .as_ref()
                .map_or(std::ptr::null(), |s| s.as_ptr()),
        }
    }
}

pub struct Message {
    pub(crate) c_msg: *mut flux_msg_t,
}

impl Message {
    pub fn new(msg_type: MessageType) -> Result<Self> {
        let msg_ptr = unsafe { flux_msg_create(msg_type.bits() as i32) };
        check_ptr(msg_ptr)?;
        Ok(Self { c_msg: msg_ptr })
    }

    pub fn try_clone(&self, copy_payload: bool) -> Result<Self> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot duplicate a message when the original message has a NULL internal pointer",
            )));
        }
        let new_msg_ptr = unsafe { flux_msg_copy(self.c_msg, copy_payload) };
        check_ptr(new_msg_ptr)?;
        Ok(Self { c_msg: new_msg_ptr })
    }

    pub fn has_flag(&self, flag: MessageFlag) -> Result<bool> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot check flags for a message when the message has a NULL internal pointer",
            )));
        }
        Ok(unsafe { flux_msg_has_flag(self.c_msg, flag.bits() as i32) })
    }

    pub fn set_flag(&mut self, flag: MessageFlag) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set flags for a message when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe { flux_msg_set_flag(self.c_msg, flag.bits() as i32) };
        check_rc(rc)
    }

    pub fn clear_flag(&mut self, flag: MessageFlag) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot clear flags for a message when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe { flux_msg_clear_flag(self.c_msg, flag.bits() as i32) };
        check_rc(rc)
    }

    pub fn set_private(&mut self) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot make a message private when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe { flux_msg_set_private(self.c_msg) };
        check_rc(rc)
    }

    pub fn is_private(&self) -> Result<bool> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot check if a message is private when the message has a NULL internal pointer",
            )));
        }
        Ok(unsafe { flux_msg_is_private(self.c_msg) })
    }

    pub fn set_streaming(&mut self) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot make a message streaming when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe { flux_msg_set_streaming(self.c_msg) };
        check_rc(rc)
    }

    pub fn is_streaming(&self) -> Result<bool> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot check if a message is streaming when the message has a NULL internal pointer",
            )));
        }
        Ok(unsafe { flux_msg_is_streaming(self.c_msg) })
    }

    pub fn set_noresponse(&mut self) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot mark a message as 'noresponse' when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe { flux_msg_set_noresponse(self.c_msg) };
        check_rc(rc)
    }

    pub fn is_noresponse(&self) -> Result<bool> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot check if a message is 'noresponse' when the message has a NULL internal pointer",
            )));
        }
        Ok(unsafe { flux_msg_is_noresponse(self.c_msg) })
    }

    pub fn is_local(&self) -> Result<bool> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot check if a message is local when the message has a NULL internal pointer",
            )));
        }
        Ok(unsafe { flux_msg_is_local(self.c_msg) })
    }

    pub fn set_topic(&mut self, topic: &str) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set topic for a message when the message has a NULL internal pointer",
            )));
        }
        let c_topic = CString::new(topic)?;
        let rc = unsafe { flux_msg_set_topic(self.c_msg, c_topic.as_ptr()) };
        check_rc(rc)
    }

    pub fn delete_topic(&mut self) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot delete topic for a message when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe { flux_msg_set_topic(self.c_msg, std::ptr::null()) };
        check_rc(rc)
    }

    pub fn get_topic(&self) -> Result<String> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get topic for a message when the message has a NULL internal pointer",
            )));
        }
        let mut c_str: *const c_char = std::ptr::null();
        let rc = unsafe { flux_msg_get_topic(self.c_msg, &mut c_str as *mut *const c_char) };
        check_rc(rc)?;
        Ok(unsafe { CStr::from_ptr(c_str).to_str()?.to_owned() })
    }

    pub fn has_payload(&self) -> Result<bool> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot check for a payload for a message when the message has a NULL internal pointer",
            )));
        }
        Ok(unsafe { flux_msg_has_payload(self.c_msg) })
    }

    pub fn set_payload(&mut self, data: &[u8]) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set a payload for a message when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe {
            flux_msg_set_payload(
                self.c_msg,
                data.as_ptr() as *const c_void,
                data.len() as i32,
            )
        };
        check_rc(rc)
    }

    pub fn set_payload_json(&mut self, data: &Value) -> Result<()> {
        let raw_payload = serde_json::to_vec(data)?;
        self.set_payload(&raw_payload)
    }

    pub fn set_payload_serializable<S: Serialize>(&mut self, data: &S) -> Result<()> {
        let raw_payload = serde_json::to_vec(data)?;
        self.set_payload(&raw_payload)
    }

    pub fn get_payload(&self) -> Result<&[u8]> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get a payload for a message when the message has a NULL internal pointer",
            )));
        }
        let mut buf: *const c_void = std::ptr::null();
        let mut size: i32 = 0;
        let rc = unsafe {
            flux_msg_get_payload(
                self.c_msg,
                &mut buf as *mut *const c_void,
                &mut size as *mut i32,
            )
        };
        check_rc(rc)?;
        Ok(unsafe { std::slice::from_raw_parts(buf as *const u8, size as usize) })
    }

    pub fn get_payload_json(&self) -> Result<Value> {
        let mut raw_payload = self.get_payload()?;
        if raw_payload.last() == Some(&0) {
            raw_payload = &raw_payload[..raw_payload.len() - 1];
        }
        Ok(serde_json::from_slice(raw_payload)?)
    }

    pub fn get_payload_deserializable<'a, D: Deserialize<'a>>(&'a self) -> Result<D> {
        let mut raw_payload = self.get_payload()?;
        if raw_payload.last() == Some(&0) {
            raw_payload = &raw_payload[..raw_payload.len() - 1];
        }
        Ok(serde_json::from_slice(raw_payload)?)
    }

    pub fn set_string(&mut self, val: &str) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set a string payload for a message when the message has a NULL internal pointer",
            )));
        }
        let c_val = CString::new(val)?;
        let rc = unsafe { flux_msg_set_string(self.c_msg, c_val.as_ptr()) };
        check_rc(rc)
    }

    pub fn get_string(&self) -> Result<&str> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set a string payload for a message when the message has a NULL internal pointer",
            )));
        }
        let mut c_str_ptr: *const c_char = std::ptr::null();
        let rc = unsafe { flux_msg_get_string(self.c_msg, &mut c_str_ptr as *mut *const c_char) };
        check_rc(rc)?;
        Ok(unsafe { CStr::from_ptr(c_str_ptr).to_str()? })
    }

    pub fn set_nodeid(&mut self, nodeid: u32) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set nodeid for a message when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe { flux_msg_set_nodeid(self.c_msg, nodeid) };
        check_rc(rc)
    }

    pub fn get_nodeid(&self) -> Result<u32> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get nodeid for a message when the message has a NULL internal pointer",
            )));
        }
        let mut nodeid: u32 = 0;
        let rc = unsafe { flux_msg_get_nodeid(self.c_msg, &mut nodeid as *mut u32) };
        check_rc(rc)?;
        Ok(nodeid)
    }

    pub fn set_cred(&mut self, userid: u32, rolemask: MessageRolemask) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set userid/rolemask for a message when the message has a NULL internal pointer",
            )));
        }
        let cred = flux_msg_cred {
            userid,
            rolemask: rolemask.bits() as u32,
        };
        let rc = unsafe { flux_msg_set_cred(self.c_msg, cred) };
        check_rc(rc)
    }

    pub fn get_cred(&self) -> Result<(u32, MessageRolemask)> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get userid/rolemask for a message when the message has a NULL internal pointer",
            )));
        }
        let mut cred: flux_msg_cred = Default::default();
        let rc = unsafe { flux_msg_get_cred(self.c_msg, &mut cred as *mut flux_msg_cred) };
        check_rc(rc)?;
        Ok((
            cred.userid,
            MessageRolemask::from_bits_retain(cred.rolemask),
        ))
    }

    pub fn authorize(&mut self, userid: u32) -> Result<bool> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot perform authorization for a message when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe { flux_msg_authorize(self.c_msg, userid) };
        if rc == -1 {
            let last_errno = std::io::Error::last_os_error();
            if let Some(raw_errno) = last_errno.raw_os_error() {
                if raw_errno == libc::EPERM {
                    return Ok(false);
                }
            }
            return Err(FluxError::System(last_errno));
        }
        Ok(true)
    }

    pub fn authorize_cred(
        &self,
        userid: u32,
        cred_userid: u32,
        cred_rolemask: MessageRolemask,
    ) -> Result<bool> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot perform authorization for a message when the message has a NULL internal pointer",
            )));
        }
        let cred = flux_msg_cred {
            userid: cred_userid,
            rolemask: cred_rolemask.bits() as u32,
        };
        let rc = unsafe { flux_msg_cred_authorize(cred, userid) };
        if rc == -1 {
            let last_errno = std::io::Error::last_os_error();
            if let Some(raw_errno) = last_errno.raw_os_error() {
                if raw_errno == libc::EPERM {
                    return Ok(false);
                }
            }
            return Err(FluxError::System(last_errno));
        }
        Ok(true)
    }

    pub fn set_error(&mut self, err: std::io::Error) -> Result<()> {
        let raw_errno = err.raw_os_error().ok_or(FluxError::Logic(String::from(
            "Cannot set errnum with a std::io::Error that does not have a raw OS error",
        )))?;
        self.set_error_raw(raw_errno)
    }

    pub fn set_error_raw(&mut self, raw_errno: i32) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set error for a message when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe { flux_msg_set_errnum(self.c_msg, raw_errno) };
        check_rc(rc)
    }

    pub fn get_error(&self) -> Result<std::io::Error> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get error for a message when the message has a NULL internal pointer",
            )));
        }
        let mut errnum: i32 = 0;
        let rc = unsafe { flux_msg_get_errnum(self.c_msg, &mut errnum as *mut i32) };
        check_rc(rc)?;
        Ok(std::io::Error::from_raw_os_error(errnum))
    }

    pub fn set_sequence(&mut self, seq: u32) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set sequence number for a message when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe { flux_msg_set_seq(self.c_msg, seq) };
        check_rc(rc)
    }

    pub fn get_sequence(&self) -> Result<u32> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get sequence number for a message when the message has a NULL internal pointer",
            )));
        }
        let mut seq: u32 = 0;
        let rc = unsafe { flux_msg_get_seq(self.c_msg, &mut seq as *mut u32) };
        check_rc(rc)?;
        Ok(seq)
    }

    // TODO add set_control/get_control methods

    pub fn match_tag(&self, tag: u32) -> Result<bool> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot match tag for a message when the message has a NULL internal pointer",
            )));
        }
        Ok(unsafe { flux_msg_cmp_matchtag(self.c_msg, tag) })
    }

    pub fn set_matchtag(&mut self, matchtag: u32) -> Result<()> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot set matchtag for a message when the message has a NULL internal pointer",
            )));
        }
        let rc = unsafe { flux_msg_set_matchtag(self.c_msg, matchtag) };
        check_rc(rc)
    }

    pub fn get_matchtag(&self) -> Result<u32> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot get matchtag for a message when the message has a NULL internal pointer",
            )));
        }
        let mut matchtag: u32 = 0;
        let rc = unsafe { flux_msg_get_matchtag(self.c_msg, &mut matchtag as *mut u32) };
        check_rc(rc)?;
        Ok(matchtag)
    }

    // TODO add wrappers for flux_msg_fprint and flux_msg_fprint_ts

    // TODO add wrappers for functions related to routes

    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.c_msg.is_null() {
            return Err(FluxError::Logic(String::from(
                "Cannot encode a message when the message has a NULL internal pointer",
            )));
        }
        let buf_size = unsafe { flux_msg_encode_size(self.c_msg) };
        if buf_size == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        } else if buf_size == 0 {
            return Ok(Vec::new());
        }
        // Pre-allocate a byte buffer of 'buf_size' bytes
        let mut buffer = vec![0u8; buf_size as usize];
        let rc = unsafe {
            flux_msg_encode(
                self.c_msg,
                buffer.as_mut_ptr() as *mut c_void,
                buf_size as usize,
            )
        };
        check_rc(rc)?;
        Ok(buffer)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        let msg_ptr = unsafe { flux_msg_decode(data.as_ptr() as *mut c_void, data.len()) };
        check_ptr(msg_ptr)?;
        Ok(Self::from(msg_ptr))
    }
}

impl Drop for Message {
    fn drop(&mut self) {
        if !self.c_msg.is_null() {
            unsafe {
                flux_msg_destroy(self.c_msg);
            }
        }
    }
}

impl Clone for Message {
    fn clone(&self) -> Self {
        if !self.c_msg.is_null() {
            unsafe {
                flux_msg_incref(self.c_msg);
            }
        }
        Self { c_msg: self.c_msg }
    }
}

impl From<*mut flux_msg_t> for Message {
    fn from(value: *mut flux_msg_t) -> Self {
        Self { c_msg: value }
    }
}

impl PartialEq<MessageMatch> for Message {
    fn eq(&self, other: &MessageMatch) -> bool {
        if self.c_msg.is_null() {
            return false;
        }
        let c_match = flux_match {
            typemask: other.typemask.unwrap_or(MessageType::NONE).bits() as i32,
            matchtag: other.matchtag.unwrap_or(FLUX_MATCHTAG_NONE),
            topic_glob: other
                .topic_glob
                .as_ref()
                .map_or(std::ptr::null(), |cstr| cstr.as_ptr()),
        };
        unsafe { flux_msg_cmp(self.c_msg, c_match) }
    }
}
