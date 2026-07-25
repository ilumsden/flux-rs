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
use crate::flux_ptr_management::{
    default_impl_as_flux_ptr, BorrowFluxPtr, FluxPtr, FromFluxPtr, FromFluxPtrNoArgs,
};

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

#[derive(Default, Clone, PartialEq, Eq)]
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
    pub(crate) c_msg: FluxPtr<flux_msg_t>,
}

impl Message {
    pub fn new(msg_type: MessageType) -> Result<Self> {
        let msg_ptr = unsafe { flux_msg_create(msg_type.bits() as i32) };
        check_ptr(msg_ptr)?;
        Ok(Self {
            c_msg: FluxPtr::create_owned(msg_ptr, flux_msg_destroy)?,
        })
    }

    pub fn try_clone(&self, copy_payload: bool) -> Result<Self> {
        let new_msg_ptr = unsafe { flux_msg_copy(self.c_msg.as_mut_ptr(), copy_payload) };
        check_ptr(new_msg_ptr)?;
        Ok(Self {
            c_msg: FluxPtr::create_owned(new_msg_ptr, flux_msg_destroy)?,
        })
    }

    pub fn has_flag(&self, flag: MessageFlag) -> bool {
        unsafe { flux_msg_has_flag(self.c_msg.as_mut_ptr(), flag.bits() as i32) }
    }

    pub fn set_flag(&mut self, flag: MessageFlag) -> Result<()> {
        let rc = unsafe { flux_msg_set_flag(self.c_msg.as_mut_ptr(), flag.bits() as i32) };
        check_rc(rc)
    }

    pub fn clear_flag(&mut self, flag: MessageFlag) -> Result<()> {
        let rc = unsafe { flux_msg_clear_flag(self.c_msg.as_mut_ptr(), flag.bits() as i32) };
        check_rc(rc)
    }

    pub fn set_private(&mut self) -> Result<()> {
        let rc = unsafe { flux_msg_set_private(self.c_msg.as_mut_ptr()) };
        check_rc(rc)
    }

    pub fn is_private(&self) -> bool {
        unsafe { flux_msg_is_private(self.c_msg.as_mut_ptr()) }
    }

    pub fn set_streaming(&mut self) -> Result<()> {
        let rc = unsafe { flux_msg_set_streaming(self.c_msg.as_mut_ptr()) };
        check_rc(rc)
    }

    pub fn is_streaming(&self) -> bool {
        unsafe { flux_msg_is_streaming(self.c_msg.as_mut_ptr()) }
    }

    pub fn set_noresponse(&mut self) -> Result<()> {
        let rc = unsafe { flux_msg_set_noresponse(self.c_msg.as_mut_ptr()) };
        check_rc(rc)
    }

    pub fn is_noresponse(&self) -> bool {
        unsafe { flux_msg_is_noresponse(self.c_msg.as_mut_ptr()) }
    }

    pub fn is_local(&self) -> bool {
        unsafe { flux_msg_is_local(self.c_msg.as_mut_ptr()) }
    }

    pub fn set_topic(&mut self, topic: &str) -> Result<()> {
        let c_topic = CString::new(topic)?;
        let rc = unsafe { flux_msg_set_topic(self.c_msg.as_mut_ptr(), c_topic.as_ptr()) };
        check_rc(rc)
    }

    pub fn delete_topic(&mut self) -> Result<()> {
        let rc = unsafe { flux_msg_set_topic(self.c_msg.as_mut_ptr(), std::ptr::null()) };
        check_rc(rc)
    }

    pub fn get_topic(&self) -> Result<String> {
        let mut c_str: *const c_char = std::ptr::null();
        let rc = unsafe {
            flux_msg_get_topic(self.c_msg.as_mut_ptr(), &mut c_str as *mut *const c_char)
        };
        check_rc(rc)?;
        Ok(unsafe { CStr::from_ptr(c_str).to_str()?.to_owned() })
    }

    pub fn has_payload(&self) -> bool {
        unsafe { flux_msg_has_payload(self.c_msg.as_mut_ptr()) }
    }

    pub fn set_payload(&mut self, data: &[u8]) -> Result<()> {
        let rc = unsafe {
            flux_msg_set_payload(
                self.c_msg.as_mut_ptr(),
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
        let mut buf: *const c_void = std::ptr::null();
        let mut size: i32 = 0;
        let rc = unsafe {
            flux_msg_get_payload(
                self.c_msg.as_mut_ptr(),
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
        let c_val = CString::new(val)?;
        let rc = unsafe { flux_msg_set_string(self.c_msg.as_mut_ptr(), c_val.as_ptr()) };
        check_rc(rc)
    }

    pub fn get_string(&self) -> Result<&str> {
        let mut c_str_ptr: *const c_char = std::ptr::null();
        let rc = unsafe {
            flux_msg_get_string(
                self.c_msg.as_mut_ptr(),
                &mut c_str_ptr as *mut *const c_char,
            )
        };
        check_rc(rc)?;
        Ok(unsafe { CStr::from_ptr(c_str_ptr).to_str()? })
    }

    pub fn set_nodeid(&mut self, nodeid: u32) -> Result<()> {
        let rc = unsafe { flux_msg_set_nodeid(self.c_msg.as_mut_ptr(), nodeid) };
        check_rc(rc)
    }

    pub fn get_nodeid(&self) -> Result<u32> {
        let mut nodeid: u32 = 0;
        let rc = unsafe { flux_msg_get_nodeid(self.c_msg.as_mut_ptr(), &mut nodeid as *mut u32) };
        check_rc(rc)?;
        Ok(nodeid)
    }

    pub fn set_cred(&mut self, userid: u32, rolemask: MessageRolemask) -> Result<()> {
        let cred = flux_msg_cred {
            userid,
            rolemask: rolemask.bits() as u32,
        };
        let rc = unsafe { flux_msg_set_cred(self.c_msg.as_mut_ptr(), cred) };
        check_rc(rc)
    }

    pub fn get_cred(&self) -> Result<(u32, MessageRolemask)> {
        let mut cred: flux_msg_cred = Default::default();
        let rc =
            unsafe { flux_msg_get_cred(self.c_msg.as_mut_ptr(), &mut cred as *mut flux_msg_cred) };
        check_rc(rc)?;
        Ok((
            cred.userid,
            MessageRolemask::from_bits_retain(cred.rolemask),
        ))
    }

    pub fn authorize(&mut self, userid: u32) -> Result<bool> {
        let rc = unsafe { flux_msg_authorize(self.c_msg.as_mut_ptr(), userid) };
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
        let rc = unsafe { flux_msg_set_errnum(self.c_msg.as_mut_ptr(), raw_errno) };
        check_rc(rc)
    }

    pub fn get_error(&self) -> Result<std::io::Error> {
        let mut errnum: i32 = 0;
        let rc = unsafe { flux_msg_get_errnum(self.c_msg.as_mut_ptr(), &mut errnum as *mut i32) };
        check_rc(rc)?;
        Ok(std::io::Error::from_raw_os_error(errnum))
    }

    pub fn set_sequence(&mut self, seq: u32) -> Result<()> {
        let rc = unsafe { flux_msg_set_seq(self.c_msg.as_mut_ptr(), seq) };
        check_rc(rc)
    }

    pub fn get_sequence(&self) -> Result<u32> {
        let mut seq: u32 = 0;
        let rc = unsafe { flux_msg_get_seq(self.c_msg.as_mut_ptr(), &mut seq as *mut u32) };
        check_rc(rc)?;
        Ok(seq)
    }

    // TODO add set_control/get_control methods

    pub fn match_tag(&self, tag: u32) -> bool {
        unsafe { flux_msg_cmp_matchtag(self.c_msg.as_mut_ptr(), tag) }
    }

    pub fn set_matchtag(&mut self, matchtag: u32) -> Result<()> {
        let rc = unsafe { flux_msg_set_matchtag(self.c_msg.as_mut_ptr(), matchtag) };
        check_rc(rc)
    }

    pub fn get_matchtag(&self) -> Result<u32> {
        let mut matchtag: u32 = 0;
        let rc =
            unsafe { flux_msg_get_matchtag(self.c_msg.as_mut_ptr(), &mut matchtag as *mut u32) };
        check_rc(rc)?;
        Ok(matchtag)
    }

    // TODO add wrappers for flux_msg_fprint and flux_msg_fprint_ts

    // TODO add wrappers for functions related to routes

    pub fn encode(&self) -> Result<Vec<u8>> {
        let buf_size = unsafe { flux_msg_encode_size(self.c_msg.as_mut_ptr()) };
        if buf_size == -1 {
            return Err(FluxError::System(std::io::Error::last_os_error()));
        } else if buf_size == 0 {
            return Ok(Vec::new());
        }
        // Pre-allocate a byte buffer of 'buf_size' bytes
        let mut buffer = vec![0u8; buf_size as usize];
        let rc = unsafe {
            flux_msg_encode(
                self.c_msg.as_mut_ptr(),
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
        unsafe { Self::from_ptr(msg_ptr) }
    }
}

impl Clone for Message {
    fn clone(&self) -> Self {
        let new_ptr = unsafe { flux_msg_incref(self.c_msg.as_mut_ptr()) };
        Self { c_msg: FluxPtr::create_owned(new_ptr as *mut flux_msg_t, flux_msg_destroy).expect("The 'flux_msg_incref' function returned NULL, which indicates that the Message being cloned has memory corruption") }
    }
}

impl PartialEq<MessageMatch> for Message {
    fn eq(&self, other: &MessageMatch) -> bool {
        let c_match = flux_match {
            typemask: other.typemask.unwrap_or(MessageType::NONE).bits() as i32,
            matchtag: other.matchtag.unwrap_or(FLUX_MATCHTAG_NONE),
            topic_glob: other
                .topic_glob
                .as_ref()
                .map_or(std::ptr::null(), |cstr| cstr.as_ptr()),
        };
        unsafe { flux_msg_cmp(self.c_msg.as_mut_ptr(), c_match) }
    }
}

unsafe impl BorrowFluxPtr for Message {
    type CType = flux_msg_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_msg: FluxPtr::create_borrowed(ptr, flux_msg_destroy)?,
        })
    }
}

unsafe impl FromFluxPtr for Message {
    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_msg: FluxPtr::create_owned(ptr, flux_msg_destroy)?,
        })
    }
}

default_impl_as_flux_ptr!(Message, flux_msg_t, c_msg);
