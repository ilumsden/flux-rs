use std::ffi::{CStr, CString, c_char, c_void};

use flux_sys::core::{
    flux_response_decode_error, flux_response_decode_raw, flux_response_derive,
    flux_response_encode_error, flux_response_encode_raw,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::FromFluxPtrNoArgs;
use crate::error::{FluxError, FluxReturnType, Result, flux_try};
use crate::msg::Message;
use crate::request::{
    DeserializedDecodedRequestResponse, JsonDecodedRequestResponse, RawDecodedRequestResponse,
    Request,
};

pub struct Response {
    pub(crate) msg: Message,
}

impl Response {
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

    pub fn decode_deserializable<'a, D: Deserialize<'a>>(
        &'a self,
    ) -> Result<DeserializedDecodedRequestResponse<'a, D>> {
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

    pub fn encode(topic: &str, data: &[u8]) -> Result<Response> {
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

    pub fn encode_json(topic: &str, data: &Value) -> Result<Response> {
        let data_vec = serde_json::to_vec(data)?;
        Self::encode(topic, &data_vec)
    }

    pub fn encode_serializable<T: Serialize>(topic: &str, data: &T) -> Result<Response> {
        let data_vec = serde_json::to_vec(data)?;
        Self::encode(topic, &data_vec)
    }

    pub fn encode_error(topic: &str, error: std::io::Error) -> Result<Response> {
        let errnum = error.raw_os_error().ok_or(FluxError::Logic(String::from("Passed a std::io::Error to encode_error that does not represent a OS error (i.e., errno). Consider using encode_raw_error")))?;
        let errmsg = error.to_string();
        Self::encode_raw_error(topic, errnum, &errmsg)
    }

    pub fn encode_raw_error(topic: &str, errnum: i32, errmsg: &str) -> Result<Response> {
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

    pub fn derive(request: &Request, error: Option<std::io::Error>) -> Result<Response> {
        let errnum = error.map(|err| err.raw_os_error().ok_or(FluxError::Logic(String::from(
            "Passed a std::io::Error to 'derive' that does not represent a OS error (i.e., errno). Consider using 'derive_raw_error'"
        )))).transpose()?;
        Self::derive_raw_error(request, errnum)
    }

    pub fn derive_raw_error(request: &Request, errnum: Option<i32>) -> Result<Response> {
        let msg_ptr = flux_try!(flux_response_derive(
            request.msg.c_msg.as_mut_ptr(),
            errnum.unwrap_or(0)
        ))?;
        Ok(Response {
            msg: unsafe { Message::from_ptr(msg_ptr)? },
        })
    }
}

impl From<Message> for Response {
    fn from(value: Message) -> Self {
        Self { msg: value }
    }
}

impl From<Response> for Message {
    fn from(value: Response) -> Self {
        value.msg
    }
}
