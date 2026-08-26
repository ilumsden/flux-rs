use std::ffi::{CStr, CString, c_char, c_void};
use std::pin::Pin;
use std::task::{Context, Poll};

use bitflags::bitflags;
use flux_sys::core::{
    FLUX_NODEID_ANY, FLUX_NODEID_UPSTREAM, FLUX_RPC_NORESPONSE, FLUX_RPC_STREAMING, flux_rpc_get,
    flux_rpc_get_matchtag, flux_rpc_get_nodeid, flux_rpc_message, flux_rpc_raw, flux_t,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, flux_try};
use crate::flux_ptr_management::{Borrowed, FromFluxPtrNoArgs, Owned, PossiblyDroppablePtr};
use crate::future::{AsyncFluxFuture, FluxFuture, OwnedFluxFuture};
use crate::handle::FluxHandle;
use crate::msg::Message;

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct RpcFlags: u32 {
        const NONE = 0;
        const NORESPONSE = FLUX_RPC_NORESPONSE;
        const STREAMING = FLUX_RPC_STREAMING;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RpcNodeId {
    Any,
    Upstream,
    Rank(u32),
}

impl RpcNodeId {
    pub const fn as_c_nodeid(&self) -> u32 {
        match self {
            RpcNodeId::Any => FLUX_NODEID_ANY,
            RpcNodeId::Upstream => FLUX_NODEID_UPSTREAM,
            RpcNodeId::Rank(r) => *r,
        }
    }
}

pub struct Rpc<'a, FhState: PossiblyDroppablePtr<flux_t>> {
    handle: Option<&'a FluxHandle<FhState>>,
    future: FluxFuture,
}

pub type OwnedRpc<'a> = Rpc<'a, Owned<flux_t>>;
pub type BorrowedRpc<'a, 'fh> = Rpc<'a, Borrowed<'fh, flux_t>>;

impl<'a, FhState: PossiblyDroppablePtr<flux_t>> Rpc<'a, FhState> {
    pub fn create(
        handle: &'a FluxHandle<FhState>,
        topic: &str,
        data: &[u8],
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Self> {
        let c_topic = CString::new(topic)?;
        let data_ptr = if data.is_empty() {
            std::ptr::null()
        } else {
            data.as_ptr()
        };
        let future_ptr = flux_try!(flux_rpc_raw(
            handle.h.as_mut_ptr(),
            c_topic.as_ptr(),
            data_ptr as *const c_void,
            data.len() as _,
            nodeid.as_c_nodeid(),
            flags.bits() as _,
        ))?;
        Ok(Self {
            handle: Some(handle),
            future: unsafe { FluxFuture::from_ptr(future_ptr)? },
        })
    }

    pub fn create_json(
        handle: &'a FluxHandle<FhState>,
        topic: &str,
        data: &Value,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Self> {
        let data_vec = serde_json::to_vec(data)?;
        Self::create(handle, topic, &data_vec, nodeid, flags)
    }

    pub fn create_serializable<T: Serialize>(
        handle: &'a FluxHandle<FhState>,
        topic: &str,
        data: &T,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Self> {
        let data_vec = serde_json::to_vec(data)?;
        Self::create(handle, topic, &data_vec, nodeid, flags)
    }

    pub fn create_message(
        handle: &'a FluxHandle<FhState>,
        msg: &Message,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Self> {
        let future_ptr = flux_try!(flux_rpc_message(
            handle.h.as_mut_ptr(),
            msg.c_msg.as_mut_ptr(),
            nodeid.as_c_nodeid(),
            flags.bits() as _,
        ))?;
        Ok(Self {
            handle: Some(handle),
            future: unsafe { FluxFuture::from_ptr(future_ptr)? },
        })
    }

    pub fn get<'s>(&'s self) -> Result<Option<&'s [u8]>> {
        let mut buf: *const c_char = std::ptr::null();
        flux_try!(flux_rpc_get(
            self.future.c_future.as_mut_ptr(),
            &mut buf as *mut *const c_char,
        ))?;
        if buf.is_null() {
            Ok(None)
        } else {
            Ok(Some(unsafe { CStr::from_ptr(buf).to_bytes() }))
        }
    }

    pub fn get_json(&self) -> Result<Option<Value>> {
        let mut raw_payload = match self.get()? {
            Some(p) => p,
            None => return Ok(None),
        };
        if raw_payload.last() == Some(&0) {
            raw_payload = &raw_payload[..raw_payload.len() - 1];
        }
        Ok(serde_json::from_slice(raw_payload)?)
    }

    pub fn get_deserializable<'s, D: Deserialize<'s>>(&'s self) -> Result<Option<D>> {
        let mut raw_payload = match self.get()? {
            Some(p) => p,
            None => return Ok(None),
        };
        if raw_payload.last() == Some(&0) {
            raw_payload = &raw_payload[..raw_payload.len() - 1];
        }
        Ok(serde_json::from_slice(raw_payload)?)
    }

    pub fn get_matchtag(&self) -> u32 {
        unsafe { flux_rpc_get_matchtag(self.future.c_future.as_mut_ptr()) }
    }

    pub fn get_nodeid(&self) -> u32 {
        unsafe { flux_rpc_get_nodeid(self.future.c_future.as_mut_ptr()) }
    }
}

impl From<OwnedFluxFuture> for Rpc<'_, Owned<flux_t>> {
    fn from(value: OwnedFluxFuture) -> Self {
        Self {
            handle: None,
            future: value,
        }
    }
}

pub struct AsyncRpc<'a, State: PossiblyDroppablePtr<flux_t>> {
    future: AsyncFluxFuture,
    handle: Option<&'a FluxHandle<State>>,
}

impl<'a, State: PossiblyDroppablePtr<flux_t>> AsyncRpc<'a, State> {
    pub fn new(sync_val: Rpc<'a, State>) -> Result<Self> {
        Ok(Self {
            future: AsyncFluxFuture::new(sync_val.future)?,
            handle: sync_val.handle,
        })
    }
}

impl<'a, State: PossiblyDroppablePtr<flux_t>> ::std::future::Future for AsyncRpc<'a, State> {
    type Output = Rpc<'a, State>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pinned_future = Pin::new(&mut self.future);
        match pinned_future.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(sync_future) => Poll::Ready(Rpc {
                future: sync_future,
                handle: self.handle,
            }),
        }
    }
}
