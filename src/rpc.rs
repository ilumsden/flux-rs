use std::ffi::{c_void, CString};

use bitflags::bitflags;
use flux_sys::core::{
    flux_rpc_get_matchtag, flux_rpc_get_nodeid, flux_rpc_get_raw, flux_rpc_message, flux_rpc_raw,
    FLUX_NODEID_ANY, FLUX_NODEID_UPSTREAM, FLUX_RPC_NORESPONSE, FLUX_RPC_STREAMING,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{check_ptr, check_rc, Result};
use crate::flux_ptr_management::FromFluxPtrNoArgs;
use crate::future::FluxFuture;
use crate::handle::FluxHandle;
use crate::msg::Message;
use crate::utils::impl_async_future_wrapper;

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

pub struct Rpc<'a> {
    handle: &'a FluxHandle,
    future: FluxFuture<'static>,
}

impl<'a> Rpc<'a> {
    pub fn create(
        handle: &'a FluxHandle,
        topic: &str,
        data: &[u8],
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Self> {
        let c_topic = CString::new(topic)?;
        let future_ptr = unsafe {
            flux_rpc_raw(
                handle.h.as_mut_ptr(),
                c_topic.as_ptr(),
                data.as_ptr() as *const c_void,
                data.len() as _,
                nodeid.as_c_nodeid(),
                flags.bits() as _,
            )
        };
        check_ptr(future_ptr)?;
        Ok(Self {
            handle,
            future: unsafe { FluxFuture::from_ptr(future_ptr)? },
        })
    }

    pub fn create_json(
        handle: &'a FluxHandle,
        topic: &str,
        data: &Value,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Self> {
        let data_vec = serde_json::to_vec(data)?;
        Self::create(handle, topic, &data_vec, nodeid, flags)
    }

    pub fn create_serializable<T: Serialize>(
        handle: &'a FluxHandle,
        topic: &str,
        data: &T,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Self> {
        let data_vec = serde_json::to_vec(data)?;
        Self::create(handle, topic, &data_vec, nodeid, flags)
    }

    pub fn create_message(
        handle: &'a FluxHandle,
        msg: &Message,
        nodeid: RpcNodeId,
        flags: RpcFlags,
    ) -> Result<Self> {
        let future_ptr = unsafe {
            flux_rpc_message(
                handle.h.as_mut_ptr(),
                msg.c_msg.as_mut_ptr(),
                nodeid.as_c_nodeid(),
                flags.bits() as _,
            )
        };
        check_ptr(future_ptr)?;
        Ok(Self {
            handle: handle,
            future: unsafe { FluxFuture::from_ptr(future_ptr)? },
        })
    }

    pub fn get(&self) -> Result<&'a [u8]> {
        let mut buf: *const c_void = std::ptr::null();
        let mut size = 0;
        let rc = unsafe {
            flux_rpc_get_raw(
                self.future.c_future.as_mut_ptr(),
                &mut buf as *mut *const c_void,
                &mut size as *mut _,
            )
        };
        check_rc(rc)?;
        Ok(unsafe { std::slice::from_raw_parts(buf as *const u8, size as usize) })
    }

    pub fn get_json(&self) -> Result<Value> {
        let mut raw_payload = self.get()?;
        if raw_payload.last() == Some(&0) {
            raw_payload = &raw_payload[..raw_payload.len() - 1];
        }
        Ok(serde_json::from_slice(raw_payload)?)
    }

    pub fn get_deserializable<D: Deserialize<'a>>(&self) -> Result<D> {
        let mut raw_payload = self.get()?;
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

impl_async_future_wrapper!(
    #[from_sync(Rpc<'a>)]
    pub struct AsyncRpc<'a> {
        #[from_sync(future)]
        future: AsyncFluxFuture,
        #[from_sync(handle)]
        handle: &'a FluxHandle,
    }
);
