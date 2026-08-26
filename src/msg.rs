use std::ffi::{CStr, CString, c_char, c_void};
use std::fmt::Display;

use bitflags::bitflags;
use flux_sys::core::{
    FLUX_MATCHTAG_NONE, FLUX_MSGFLAG_NORESPONSE, FLUX_MSGFLAG_PAYLOAD, FLUX_MSGFLAG_PRIVATE,
    FLUX_MSGFLAG_ROUTE, FLUX_MSGFLAG_STREAMING, FLUX_MSGFLAG_TOPIC, FLUX_MSGFLAG_UPSTREAM,
    FLUX_MSGFLAG_USER1, FLUX_MSGTYPE_ANY, FLUX_MSGTYPE_CONTROL, FLUX_MSGTYPE_EVENT,
    FLUX_MSGTYPE_MASK, FLUX_MSGTYPE_REQUEST, FLUX_MSGTYPE_RESPONSE, FLUX_ROLE_ALL, FLUX_ROLE_LOCAL,
    FLUX_ROLE_NONE, FLUX_ROLE_OWNER, FLUX_ROLE_USER, flux_match, flux_msg_authorize,
    flux_msg_clear_flag, flux_msg_cmp, flux_msg_cmp_matchtag, flux_msg_copy, flux_msg_create,
    flux_msg_cred, flux_msg_cred_authorize, flux_msg_decode, flux_msg_destroy, flux_msg_encode,
    flux_msg_encode_size, flux_msg_get_cred, flux_msg_get_errnum, flux_msg_get_matchtag,
    flux_msg_get_nodeid, flux_msg_get_payload, flux_msg_get_seq, flux_msg_get_string,
    flux_msg_get_topic, flux_msg_has_flag, flux_msg_has_payload, flux_msg_incref,
    flux_msg_is_local, flux_msg_is_noresponse, flux_msg_is_private, flux_msg_is_streaming,
    flux_msg_set_cred, flux_msg_set_errnum, flux_msg_set_flag, flux_msg_set_matchtag,
    flux_msg_set_nodeid, flux_msg_set_noresponse, flux_msg_set_payload, flux_msg_set_private,
    flux_msg_set_seq, flux_msg_set_streaming, flux_msg_set_string, flux_msg_set_topic, flux_msg_t,
    flux_msg_typestr,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{FluxError, Result, flux_try};
use crate::flux_ptr_management::{
    AsFluxPtr, BorrowFluxPtr, Borrowed, FluxPtr, FromFluxPtr, FromFluxPtrNoArgs, IntoFluxPtr,
    Owned, PossiblyDroppablePtr, define_as_flux_ptr_body, define_into_flux_ptr_body,
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
        let typestr_ptr = unsafe { flux_msg_typestr(self.bits() as _) };
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

#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct MessageMatch {
    pub(crate) typemask: Option<MessageType>,
    pub(crate) matchtag: Option<u32>,
    pub(crate) topic_glob: Option<CString>,
}

impl MessageMatch {
    pub const ANY: Self = MessageMatch {
        typemask: None,
        matchtag: None,
        topic_glob: None,
    };

    pub fn new(
        typemask: Option<MessageType>,
        matchtag: Option<u32>,
        topic_glob: Option<&str>,
    ) -> Result<Self> {
        let c_topic_glob = topic_glob.map(CString::new).transpose()?;
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
        let c_topic_glob = topic_glob.map(CString::new).transpose()?;
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
            typemask: Some(MessageType::from_bits_retain(value.typemask as _)),
            matchtag: Some(value.matchtag),
            topic_glob: ru_topic_glob,
        })
    }
}

impl From<&MessageMatch> for flux_match {
    fn from(value: &MessageMatch) -> Self {
        flux_match {
            typemask: value
                .typemask
                .map_or(FLUX_MSGTYPE_ANY as i32, |t| t.bits() as _),
            matchtag: value.matchtag.unwrap_or(FLUX_MATCHTAG_NONE),
            topic_glob: value
                .topic_glob
                .as_ref()
                .map_or(std::ptr::null(), |s| s.as_ptr()),
        }
    }
}

pub struct Message<State: PossiblyDroppablePtr<flux_msg_t> = Owned<flux_msg_t>> {
    pub(crate) c_msg: FluxPtr<flux_msg_t, State>,
}

pub type OwnedMessage = Message<Owned<flux_msg_t>>;
pub type BorrowedMessage<'a> = Message<Borrowed<'a, flux_msg_t>>;

impl OwnedMessage {
    pub fn new(msg_type: MessageType) -> Result<Self> {
        let msg_ptr = flux_try!(flux_msg_create(msg_type.bits() as _))?;
        Ok(Self {
            c_msg: FluxPtr::create_owned(msg_ptr, flux_msg_destroy)?,
        })
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        let msg_ptr = flux_try!(flux_msg_decode(data.as_ptr() as *mut c_void, data.len()))?;
        unsafe { Self::from_ptr(msg_ptr) }
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> Message<State> {
    pub fn try_clone(&self, copy_payload: bool) -> Result<OwnedMessage> {
        let new_msg_ptr = flux_try!(flux_msg_copy(self.c_msg.as_mut_ptr(), copy_payload))?;
        Ok(Message {
            c_msg: FluxPtr::create_owned(new_msg_ptr, flux_msg_destroy)?,
        })
    }

    pub fn to_owned(&self) -> OwnedMessage {
        let new_ptr = unsafe { flux_msg_incref(self.c_msg.as_mut_ptr()) };
        Message { c_msg: FluxPtr::create_owned(new_ptr as *mut flux_msg_t, flux_msg_destroy).expect("The 'flux_msg_incref' function returned NULL, which indicates that the Message being cloned has memory corruption") }
    }

    pub fn has_flag(&self, flag: MessageFlag) -> bool {
        unsafe { flux_msg_has_flag(self.c_msg.as_mut_ptr(), flag.bits() as _) }
    }

    pub fn set_flag(&mut self, flag: MessageFlag) -> Result<()> {
        flux_try!(empty_ok flux_msg_set_flag(self.c_msg.as_mut_ptr(), flag.bits() as _))
    }

    pub fn clear_flag(&mut self, flag: MessageFlag) -> Result<()> {
        flux_try!(empty_ok flux_msg_clear_flag(self.c_msg.as_mut_ptr(), flag.bits() as _))
    }

    pub fn set_private(&mut self) -> Result<()> {
        flux_try!(empty_ok flux_msg_set_private(self.c_msg.as_mut_ptr()))
    }

    pub fn is_private(&self) -> bool {
        unsafe { flux_msg_is_private(self.c_msg.as_mut_ptr()) }
    }

    pub fn set_streaming(&mut self) -> Result<()> {
        flux_try!(empty_ok flux_msg_set_streaming(self.c_msg.as_mut_ptr()))
    }

    pub fn is_streaming(&self) -> bool {
        unsafe { flux_msg_is_streaming(self.c_msg.as_mut_ptr()) }
    }

    pub fn set_noresponse(&mut self) -> Result<()> {
        flux_try!(empty_ok flux_msg_set_noresponse(self.c_msg.as_mut_ptr()))
    }

    pub fn is_noresponse(&self) -> bool {
        unsafe { flux_msg_is_noresponse(self.c_msg.as_mut_ptr()) }
    }

    pub fn is_local(&self) -> bool {
        unsafe { flux_msg_is_local(self.c_msg.as_mut_ptr()) }
    }

    pub fn set_topic(&mut self, topic: &str) -> Result<()> {
        let c_topic = CString::new(topic)?;
        flux_try!(empty_ok flux_msg_set_topic(
            self.c_msg.as_mut_ptr(),
            c_topic.as_ptr()
        ))
    }

    pub fn delete_topic(&mut self) -> Result<()> {
        flux_try!(empty_ok flux_msg_set_topic(self.c_msg.as_mut_ptr(), std::ptr::null()))
    }

    pub fn get_topic(&self) -> Result<String> {
        let mut c_str: *const c_char = std::ptr::null();
        flux_try!(flux_msg_get_topic(
            self.c_msg.as_mut_ptr(),
            &mut c_str as *mut *const c_char
        ))?;
        Ok(unsafe { CStr::from_ptr(c_str).to_str()?.to_owned() })
    }

    pub fn has_payload(&self) -> bool {
        unsafe { flux_msg_has_payload(self.c_msg.as_mut_ptr()) }
    }

    pub fn set_payload(&mut self, data: &[u8]) -> Result<()> {
        flux_try!(empty_ok
            flux_msg_set_payload(
                self.c_msg.as_mut_ptr(),
                data.as_ptr() as *const c_void,
                data.len() as _,
            )
        )
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
        let mut size = 0;
        flux_try!(flux_msg_get_payload(
            self.c_msg.as_mut_ptr(),
            &mut buf as *mut *const c_void,
            &mut size,
        ))?;
        Ok(unsafe { std::slice::from_raw_parts(buf as *const u8, size as _) })
    }

    pub fn get_payload_json(&self) -> Result<Value> {
        let mut raw_payload = self.get_payload()?;
        if raw_payload.last() == Some(&0) {
            raw_payload = &raw_payload[..raw_payload.len() - 1];
        }
        Ok(serde_json::from_slice(raw_payload)?)
    }

    pub fn get_payload_deserializable<'b, D: Deserialize<'b>>(&'b self) -> Result<D> {
        let mut raw_payload = self.get_payload()?;
        if raw_payload.last() == Some(&0) {
            raw_payload = &raw_payload[..raw_payload.len() - 1];
        }
        Ok(serde_json::from_slice(raw_payload)?)
    }

    pub fn set_string(&mut self, val: &str) -> Result<()> {
        let c_val = CString::new(val)?;
        flux_try!(empty_ok flux_msg_set_string(self.c_msg.as_mut_ptr(), c_val.as_ptr()))
    }

    pub fn get_string(&self) -> Result<&str> {
        let mut c_str_ptr: *const c_char = std::ptr::null();
        flux_try!(flux_msg_get_string(
            self.c_msg.as_mut_ptr(),
            &mut c_str_ptr as *mut *const c_char,
        ))?;
        Ok(unsafe { CStr::from_ptr(c_str_ptr).to_str()? })
    }

    pub fn set_nodeid(&mut self, nodeid: u32) -> Result<()> {
        flux_try!(empty_ok flux_msg_set_nodeid(self.c_msg.as_mut_ptr(), nodeid))
    }

    pub fn get_nodeid(&self) -> Result<u32> {
        let mut nodeid: u32 = 0;
        flux_try!(flux_msg_get_nodeid(
            self.c_msg.as_mut_ptr(),
            &mut nodeid as *mut _
        ))?;
        Ok(nodeid)
    }

    pub fn set_cred(&mut self, userid: u32, rolemask: MessageRolemask) -> Result<()> {
        let cred = flux_msg_cred {
            userid,
            rolemask: rolemask.bits() as _,
        };
        flux_try!(empty_ok flux_msg_set_cred(self.c_msg.as_mut_ptr(), cred))
    }

    pub fn get_cred(&self) -> Result<(u32, MessageRolemask)> {
        let mut cred: flux_msg_cred = Default::default();
        flux_try!(flux_msg_get_cred(
            self.c_msg.as_mut_ptr(),
            &mut cred as *mut flux_msg_cred
        ))?;
        Ok((
            cred.userid,
            MessageRolemask::from_bits_retain(cred.rolemask),
        ))
    }

    pub fn authorize(&mut self, userid: u32) -> Result<bool> {
        let rc = unsafe { flux_msg_authorize(self.c_msg.as_mut_ptr(), userid) };
        if rc == -1 {
            let last_errno = std::io::Error::last_os_error();
            if let Some(raw_errno) = last_errno.raw_os_error()
                && raw_errno == libc::EPERM
            {
                return Ok(false);
            }
            return Err(FluxError::System("flux_msg_authorize", last_errno));
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
            rolemask: cred_rolemask.bits() as _,
        };
        let rc = unsafe { flux_msg_cred_authorize(cred, userid) };
        if rc == -1 {
            let last_errno = std::io::Error::last_os_error();
            if let Some(raw_errno) = last_errno.raw_os_error()
                && raw_errno == libc::EPERM
            {
                return Ok(false);
            }
            return Err(FluxError::System("flux_msg_cred_authorize", last_errno));
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
        flux_try!(empty_ok flux_msg_set_errnum(self.c_msg.as_mut_ptr(), raw_errno))
    }

    pub fn get_error(&self) -> Result<std::io::Error> {
        let mut errnum: i32 = 0;
        flux_try!(flux_msg_get_errnum(
            self.c_msg.as_mut_ptr(),
            &mut errnum as *mut _
        ))?;
        Ok(std::io::Error::from_raw_os_error(errnum))
    }

    pub fn set_sequence(&mut self, seq: u32) -> Result<()> {
        flux_try!(empty_ok flux_msg_set_seq(self.c_msg.as_mut_ptr(), seq))
    }

    pub fn get_sequence(&self) -> Result<u32> {
        let mut seq: u32 = 0;
        flux_try!(flux_msg_get_seq(
            self.c_msg.as_mut_ptr(),
            &mut seq as *mut _
        ))?;
        Ok(seq)
    }

    // TODO add set_control/get_control methods

    pub fn match_tag(&self, tag: u32) -> bool {
        unsafe { flux_msg_cmp_matchtag(self.c_msg.as_mut_ptr(), tag) }
    }

    pub fn set_matchtag(&mut self, matchtag: u32) -> Result<()> {
        flux_try!(empty_ok flux_msg_set_matchtag(self.c_msg.as_mut_ptr(), matchtag))
    }

    pub fn get_matchtag(&self) -> Result<u32> {
        let mut matchtag: u32 = 0;
        flux_try!(flux_msg_get_matchtag(
            self.c_msg.as_mut_ptr(),
            &mut matchtag as *mut _
        ))?;
        Ok(matchtag)
    }

    // TODO add wrappers for flux_msg_fprint and flux_msg_fprint_ts

    // TODO add wrappers for functions related to routes

    pub fn encode(&self) -> Result<Vec<u8>> {
        let buf_size = flux_try!(flux_msg_encode_size(self.c_msg.as_mut_ptr()))?;
        if buf_size == 0 {
            return Ok(Vec::new());
        }
        // Pre-allocate a byte buffer of 'buf_size' bytes
        let mut buffer = vec![0u8; buf_size as usize];
        flux_try!(flux_msg_encode(
            self.c_msg.as_mut_ptr(),
            buffer.as_mut_ptr() as *mut c_void,
            buf_size as _,
        ))?;
        Ok(buffer)
    }
}

impl Clone for OwnedMessage {
    fn clone(&self) -> Self {
        self.to_owned()
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> TryFrom<&Message<State>> for OwnedMessage {
    type Error = FluxError;

    fn try_from(value: &Message<State>) -> std::prelude::v1::Result<Self, Self::Error> {
        value.try_clone(true)
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> PartialEq<MessageMatch> for Message<State> {
    fn eq(&self, other: &MessageMatch) -> bool {
        let c_match = flux_match {
            typemask: other.typemask.unwrap_or(MessageType::NONE).bits() as _,
            matchtag: other.matchtag.unwrap_or(FLUX_MATCHTAG_NONE),
            topic_glob: other
                .topic_glob
                .as_ref()
                .map_or(std::ptr::null(), |cstr| cstr.as_ptr()),
        };
        unsafe { flux_msg_cmp(self.c_msg.as_mut_ptr(), c_match) }
    }
}

unsafe impl<'a> BorrowFluxPtr for BorrowedMessage<'a> {
    type CType = flux_msg_t;
    type FromRawArgs = ();

    unsafe fn borrow_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_msg: FluxPtr::create_borrowed(ptr)?,
        })
    }
}

unsafe impl FromFluxPtr for OwnedMessage {
    type CType = flux_msg_t;
    type FromRawArgs = ();

    unsafe fn from_raw(ptr: *mut Self::CType, _args: Self::FromRawArgs) -> Result<Self> {
        Ok(Self {
            c_msg: FluxPtr::create_owned(ptr, flux_msg_destroy)?,
        })
    }
}

unsafe impl<State: PossiblyDroppablePtr<flux_msg_t>> AsFluxPtr for Message<State> {
    define_as_flux_ptr_body!(flux_msg_t, c_msg);
}

unsafe impl IntoFluxPtr for OwnedMessage {
    define_into_flux_ptr_body!(c_msg);
}
