use std::ffi::{CStr, CString, c_char, c_void};
use std::ops::{Deref, DerefMut};

use flux_sys::core::{flux_msg_t, flux_request_decode_raw, flux_request_encode_raw};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{FluxReturnType, Result, flux_try};
use crate::flux_ptr_management::{Borrowed, FromFluxPtrNoArgs, Owned, PossiblyDroppablePtr};
use crate::msg::Message;

#[derive(Debug)]
pub struct RawDecodedRequestResponse<'a> {
    pub topic: &'a str,
    pub payload: Option<&'a [u8]>,
}

#[derive(Debug)]
pub struct JsonDecodedRequestResponse<'a> {
    pub topic: &'a str,
    pub payload: Option<Value>,
}

pub struct DeserializedDecodedRequestResponse<'a, D: Deserialize<'a>> {
    pub topic: &'a str,
    pub payload: Option<D>,
}

pub struct Request<State: PossiblyDroppablePtr<flux_msg_t> = Owned<flux_msg_t>> {
    pub(crate) msg: Message<State>,
}

pub type OwnedRequest = Request<Owned<flux_msg_t>>;
pub type BorrowedRequest<'a> = Request<Borrowed<'a, flux_msg_t>>;

impl OwnedRequest {
    pub fn encode(topic: &str, data: &[u8]) -> Result<Self> {
        let c_topic = CString::new(topic)?;
        let msg_ptr = flux_try!(flux_request_encode_raw(
            c_topic.as_ptr(),
            data.as_ptr() as *const c_void,
            data.len() as _,
        ))?;
        Ok(Request {
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
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> Request<State> {
    pub fn decode(&self) -> Result<RawDecodedRequestResponse<'_>> {
        let mut topic_ptr: *const c_char = std::ptr::null_mut();
        let mut data_ptr: *const c_void = std::ptr::null_mut();
        let mut size = 0;
        flux_try!(flux_request_decode_raw(
            self.msg.c_msg.as_mut_ptr(),
            &mut topic_ptr as *mut *const c_char,
            &mut data_ptr as *mut *const c_void,
            &mut size,
        ))?;
        FluxReturnType::check_flux_return(topic_ptr, "flux_request_decode_raw")?;
        let topic_str = unsafe { CStr::from_ptr(topic_ptr).to_str()? };
        let decoded_payload = if data_ptr.is_null() {
            None
        } else {
            Some(unsafe { std::slice::from_raw_parts(data_ptr as *const u8, size as _) })
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

impl<State: PossiblyDroppablePtr<flux_msg_t>> From<Message<State>> for Request<State> {
    fn from(value: Message<State>) -> Self {
        Self { msg: value }
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> From<Request<State>> for Message<State> {
    fn from(value: Request<State>) -> Self {
        value.msg
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> Deref for Request<State> {
    type Target = Message<State>;

    fn deref(&self) -> &Self::Target {
        &self.msg
    }
}

impl<State: PossiblyDroppablePtr<flux_msg_t>> DerefMut for Request<State> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.msg
    }
}
