use std::ffi::{CStr, CString, c_char, c_void};
use std::ops::{Deref, DerefMut};

use flux_sys::core::{
    flux_msg_t, flux_response_decode_error, flux_response_decode_raw, flux_response_derive,
    flux_response_encode_error, flux_response_encode_raw,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{FluxError, FluxReturnType, Result, flux_try};
use crate::flux_ptr_management::{Borrowed, FromFluxPtrNoArgs, Owned, PossiblyDroppablePtr};
use crate::msg::Message;
use crate::request::{
    DeserializedDecodedRequestResponse, JsonDecodedRequestResponse, RawDecodedRequestResponse,
    Request,
};

pub struct Response<State: PossiblyDroppablePtr<flux_msg_t> = Owned<flux_msg_t>> {
    pub(crate) msg: Message<State>,
}

pub type OwnedResponse = Response<Owned<flux_msg_t>>;
pub type BorrowedResponse<'a> = Response<Borrowed<'a, flux_msg_t>>;

impl OwnedResponse {
    pub fn encode(topic: &str, data: &[u8]) -> Result<Self> {
        let c_topic = CString::new(topic)?;
        let msg_ptr = flux_try!(flux_response_encode_raw(
            c_topic.as_ptr(),
            data.as_ptr() as *const c_void,
            data.len() as _,
        ))?;
        Ok(Response {
            msg: unsafe { Message::from_ptr(msg_ptr)? },
        })
    }

    pub fn encode_json(topic: &str, data: &Value) -> Result<Self> {
        let data_vec = serde_json::to_vec(data)?;
        Self::encode(topic, &data_vec)
    }

    pub fn encode_serializable<T: Serialize>(topic: &str, data: &T) -> Result<Self> {
        let data_vec = serde_json::to_vec(data)?;
        Self::encode(topic, &data_vec)
    }

    pub fn encode_error(topic: &str, error: std::io::Error) -> Result<Self> {
        let errnum = error.raw_os_error().ok_or(FluxError::Logic(String::from("Passed a std::io::Error to encode_error that does not represent a OS error (i.e., errno). Consider using encode_raw_error")))?;
        let errmsg = error.to_string();
        Self::encode_raw_error(topic, errnum, &errmsg)
    }

    pub fn encode_raw_error(topic: &str, errnum: i32, errmsg: &str) -> Result<Self> {
        let c_topic = CString::new(topic)?;
        let c_errmsg = CString::new(errmsg)?;
        let msg_ptr = flux_try!(flux_response_encode_error(
            c_topic.as_ptr(),
            errnum,
            c_errmsg.as_ptr()
        ))?;
        Ok(Response {
            msg: unsafe { Message::from_ptr(msg_ptr)? },
        })
    }

    pub fn derive<ReqState>(
        request: &Request<ReqState>,
        error: Option<std::io::Error>,
    ) -> Result<Self>
    where
        ReqState: PossiblyDroppablePtr<flux_msg_t>,
    {
        let errnum = error.map(|err| err.raw_os_error().ok_or(FluxError::Logic(String::from(
            "Passed a std::io::Error to 'derive' that does not represent a OS error (i.e., errno). Consider using 'derive_raw_error'"
        )))).transpose()?;
        Self::derive_raw_error(request, errnum)
    }

    pub fn derive_raw_error<ReqState>(
        request: &Request<ReqState>,
        errnum: Option<i32>,
    ) -> Result<Self>
    where
        ReqState: PossiblyDroppablePtr<flux_msg_t>,
    {
        let msg_ptr = flux_try!(flux_response_derive(
            request.msg.c_msg.as_mut_ptr(),
            errnum.unwrap_or(0)
        ))?;
        Ok(Response {
            msg: unsafe { Message::from_ptr(msg_ptr)? },
        })
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> Response<State> {
    pub fn decode(&self) -> Result<RawDecodedRequestResponse<'_>> {
        let mut topic: *const c_char = std::ptr::null_mut();
        let mut data: *const c_void = std::ptr::null_mut();
        let mut len = 0;
        let mut rc = unsafe {
            flux_response_decode_raw(
                self.msg.c_msg.as_mut_ptr(),
                &mut topic as *mut *const c_char,
                &mut data as *mut *const c_void,
                &mut len,
            )
        };
        if rc == -1 {
            let last_os_error = std::io::Error::last_os_error();
            let mut errmsg_ptr: *const c_char = std::ptr::null_mut();
            rc = unsafe {
                flux_response_decode_error(
                    self.msg.c_msg.as_mut_ptr(),
                    &mut errmsg_ptr as *mut *const c_char,
                )
            };
            if rc == -1 || errmsg_ptr.is_null() {
                return Err(FluxError::System("flux_response_decode_raw", last_os_error));
            } else {
                return Err(FluxError::RequestResponseError(last_os_error, unsafe {
                    CStr::from_ptr(errmsg_ptr as *const c_char)
                        .to_str()?
                        .to_owned()
                }));
            }
        }
        FluxReturnType::check_flux_return(topic, "flux_response_decode_raw")?;
        let topic_str = unsafe { CStr::from_ptr(topic).to_str()? };
        let decoded_payload = if data.is_null() {
            None
        } else {
            Some(unsafe { std::slice::from_raw_parts(data as *const u8, len as _) })
        };
        Ok(RawDecodedRequestResponse {
            topic: topic_str,
            payload: decoded_payload,
        })
    }

    pub fn decode_json(&self) -> Result<JsonDecodedRequestResponse<'_>> {
        let RawDecodedRequestResponse {
            topic: decoded_topic,
            payload: decoded_payload,
        } = self.decode()?;
        let json_payload = decoded_payload
            .map(|mut dp| {
                if dp.last() == Some(&0) {
                    dp = &dp[..dp.len() - 1];
                }
                serde_json::from_slice(dp)
            })
            .transpose()?;
        Ok(JsonDecodedRequestResponse {
            topic: decoded_topic,
            payload: json_payload,
        })
    }

    pub fn decode_deserializable<D: for<'de> Deserialize<'de>>(
        &self,
    ) -> Result<DeserializedDecodedRequestResponse<'_, D>> {
        let RawDecodedRequestResponse {
            topic: decoded_topic,
            payload: decoded_payload,
        } = self.decode()?;
        let json_payload = decoded_payload
            .map(|mut dp| {
                if dp.last() == Some(&0) {
                    dp = &dp[..dp.len() - 1];
                }
                serde_json::from_slice(dp)
            })
            .transpose()?;
        Ok(DeserializedDecodedRequestResponse {
            topic: decoded_topic,
            payload: json_payload,
        })
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> From<Message<State>> for Response<State> {
    fn from(value: Message<State>) -> Self {
        Self { msg: value }
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> From<Response<State>> for Message<State> {
    fn from(value: Response<State>) -> Self {
        value.msg
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> Deref for Response<State> {
    type Target = Message<State>;

    fn deref(&self) -> &Self::Target {
        &self.msg
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> DerefMut for Response<State> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.msg
    }
}
