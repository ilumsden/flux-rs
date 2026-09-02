use std::ffi::{CStr, CString, c_char, c_void};
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll};

use bitflags::bitflags;
use flux_sys::core::{
    FLUX_NODEID_ANY, FLUX_NODEID_UPSTREAM, FLUX_RPC_NORESPONSE, FLUX_RPC_STREAMING, flux_rpc_get,
    flux_rpc_get_matchtag, flux_rpc_get_nodeid, flux_rpc_get_raw, flux_rpc_message, flux_rpc_raw,
    flux_t,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{FluxError, Result, flux_try};
use crate::flux_ptr_management::{Borrowed, FromFluxPtrNoArgs, Owned, PossiblyDroppablePtr};
use crate::future::{AsyncFluxFuture, FluxFuture, OwnedFluxFuture};
use crate::handle::FluxHandle;
use crate::msg::Message;

/// A utility function used to mark a branch as unlikely to be taken.
///
/// This function is intended to help the compiler in optimization, similar
/// to C/C++'s __builtin_expect.
///
/// This should be replacewd with the `likely` and `unlikely` functions once they
/// are stabilized.
#[cold]
fn mark_branch_as_unlikely() {}

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
    is_streaming: bool,
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
            is_streaming: flags.contains(RpcFlags::STREAMING),
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
            is_streaming: flags.contains(RpcFlags::STREAMING),
        })
    }

    pub fn get_raw(&self) -> Result<Option<&[u8]>> {
        let mut buf: *const c_void = std::ptr::null();
        let mut len: usize = 0;
        if let Err(e) = flux_try!(flux_rpc_get_raw(
            self.future.c_future.as_mut_ptr(),
            &mut buf as *mut *const c_void,
            &mut len as *mut _
        )) {
            if let FluxError::System(_, io_err) = &e
                && self.is_streaming
                && io_err.raw_os_error() == Some(libc::ENODATA)
            {
                return Err(FluxError::EndOfStreamRpc);
            } else {
                return Err(e);
            }
        }
        if buf.is_null() {
            // This branch is included for completeness, but realistically
            // it should never be reachable because flux_rpc_get_raw errors
            // when there is no payload
            mark_branch_as_unlikely();
            Ok(None)
        } else {
            Ok(Some(unsafe { CStr::from_ptr(buf as *const _).to_bytes() }))
        }
    }

    pub fn get(&self) -> Result<Option<&[u8]>> {
        let mut buf: *const c_char = std::ptr::null();
        if let Err(e) = flux_try!(flux_rpc_get(
            self.future.c_future.as_mut_ptr(),
            &mut buf as *mut *const c_char,
        )) {
            if let FluxError::System(_, io_err) = &e
                && self.is_streaming
                && io_err.raw_os_error() == Some(libc::ENODATA)
            {
                return Err(FluxError::EndOfStreamRpc);
            } else {
                return Err(e);
            }
        }
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
        println!("Raw payload is {:?}", raw_payload);
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

    pub fn get_string(&self) -> Result<Option<&str>> {
        let raw_payload = match self.get()? {
            Some(p) => p,
            None => return Ok(None),
        };
        let string_slice = std::str::from_utf8(raw_payload)?;
        Ok(Some(string_slice.trim_end_matches('\0')))
    }

    pub fn get_matchtag(&self) -> u32 {
        unsafe { flux_rpc_get_matchtag(self.future.c_future.as_mut_ptr()) }
    }

    pub fn get_nodeid(&self) -> u32 {
        unsafe { flux_rpc_get_nodeid(self.future.c_future.as_mut_ptr()) }
    }
}

impl<'a, T: PossiblyDroppablePtr<flux_t>> Deref for Rpc<'a, T> {
    type Target = FluxFuture;

    fn deref(&self) -> &Self::Target {
        &self.future
    }
}

impl<'a, T: PossiblyDroppablePtr<flux_t>> DerefMut for Rpc<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.future
    }
}

impl From<OwnedFluxFuture> for Rpc<'_, Owned<flux_t>> {
    fn from(value: OwnedFluxFuture) -> Self {
        Self {
            handle: None,
            future: value,
            is_streaming: false,
        }
    }
}

pub struct AsyncRpc<'a, State: PossiblyDroppablePtr<flux_t>> {
    future: AsyncFluxFuture,
    handle: Option<&'a FluxHandle<State>>,
    is_streaming: bool,
}

impl<'a, State: PossiblyDroppablePtr<flux_t>> AsyncRpc<'a, State> {
    pub fn new(sync_val: Rpc<'a, State>) -> Result<Self> {
        Ok(Self {
            future: AsyncFluxFuture::new(sync_val.future)?,
            handle: sync_val.handle,
            is_streaming: sync_val.is_streaming,
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
                is_streaming: self.is_streaming,
            }),
        }
    }
}
