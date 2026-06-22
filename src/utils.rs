use crate::error::{FluxError, Result};

pub(crate) fn parse_fsd(fsd_string: &str) -> Result<f64> {
    if matches!(fsd_string, "inf" | "infinity" | "INF" | "INFINITY") {
        return Ok(f64::INFINITY);
    }
    let (val_str, unit_multiplier) = if fsd_string.ends_with("ms") {
        (&fsd_string[..fsd_string.len() - 2], (1_f64 / 1000_f64))
    } else if fsd_string.ends_with('s') {
        (&fsd_string[..fsd_string.len() - 1], 1_f64)
    } else if fsd_string.ends_with('m') {
        (&fsd_string[..fsd_string.len() - 1], 60_f64)
    } else if fsd_string.ends_with('h') {
        (&fsd_string[..fsd_string.len() - 1], 3600_f64)
    } else if fsd_string.ends_with('d') {
        (&fsd_string[..fsd_string.len() - 1], 86400_f64)
    } else {
        (fsd_string, 1_f64)
    };
    let value: f64 = val_str.parse()?;
    let seconds = value * unit_multiplier;
    if seconds < 0.0 || seconds.is_nan() || seconds.is_infinite() {
        return Err(FluxError::Logic("The provided Flux Standard Duration produced a negative, NaN, or Infinite number of seconds".to_string()));
    }
    Ok(seconds)
}

macro_rules! impl_serde_repr_str {
    ($obj_type:ty) => {
        impl ::std::fmt::Debug for $obj_type {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let type_string = stringify!($obj_type);
                match serde_json::to_string(self) {
                    Ok(s) => write!(f, "{}({})", type_string, s),
                    Err(e) => write!(f, "\"Invalid repr for {}: {}\"", type_string, e),
                }
            }
        }

        impl ::std::fmt::Display for $obj_type {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let type_string = stringify!($obj_type);
                match serde_json::to_string(self) {
                    Ok(s) => write!(f, "{}", s),
                    Err(e) => write!(f, "\"Invalid repr for {}: {}\"", type_string, e),
                }
            }
        }
    };
    (no_debug $obj_type:ty) => {
        impl ::std::fmt::Display for $obj_type {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let type_string = stringify!($obj_type);
                match serde_json::to_string(self) {
                    Ok(s) => write!(f, "{}", s),
                    Err(e) => write!(f, "\"Invalid repr for {}: {}\"", type_string, e),
                }
            }
        }
    };
    (no_display $obj_type:ty) => {
        impl ::std::fmt::Debug for $obj_type {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let type_string = stringify!($obj_type);
                match serde_json::to_string(self) {
                    Ok(s) => write!(f, "{}({})", type_string, s),
                    Err(e) => write!(f, "\"Invalid repr for {}: {}\"", type_string, e),
                }
            }
        }
    };
}

macro_rules! impl_async_future_wrapper {
    (@to_sync_setter $self:ident, $field_name:ident, Clone) => {
        $self.$field_name.clone()
    };
    (@to_sync_setter $self:ident, $field_name:ident, $copier:expr) => {
        $copier(&$self.$field_name)
    };
    (@to_sync_setter $self:ident, $field_name:ident) => {
        $self.$field_name
    };
    (
        #[from_sync($sync_type:ident $( < $($sync_lt:lifetime),+ > )?)]
        $async_vis:vis struct $async_struct_type:ident $( < $($async_lt:lifetime),+ > )? {
            #[from_sync($sync_future_field_name:ident)] $async_future_field_name:ident: AsyncFluxFuture,
            $(
                #[from_sync($sync_field_name:ident)]
                $( #[to_sync($copier:expr)] )?
                $( #[to_sync_action($action:ident)] )?
                $field_name:ident: $field_type:ty
            ),*$(,)?
        }
    ) => {
        $async_vis struct $async_struct_type $( < $($async_lt),+ > )? {
            $async_future_field_name: $crate::future::AsyncFluxFuture,
            $( $field_name: $field_type ),*
        }

        impl $( < $($async_lt),+ > )? $async_struct_type $( < $($async_lt),+ > )? {
            $async_vis fn new(sync_val: $sync_type $( < $($sync_lt),+ > )?) -> $crate::error::Result<$async_struct_type $( < $($async_lt),+ > )?> {
                Ok(Self {
                    $async_future_field_name: $crate::future::AsyncFluxFuture::new(sync_val.$sync_future_field_name)?,
                    $($field_name: sync_val.$sync_field_name),*
                })
            }
        }

        impl $( < $($async_lt),+ > )? ::std::future::Future for $async_struct_type $( < $($async_lt),+ > )? {
            type Output = $sync_type $( < $($sync_lt),+ > )?;

            fn poll(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context<'_>,
            ) -> ::std::task::Poll<Self::Output> {
                let pinned_future = ::std::pin::Pin::new(&mut self.$async_future_field_name);
                match pinned_future.poll(cx) {
                    ::std::task::Poll::Pending => ::std::task::Poll::Pending,
                    ::std::task::Poll::Ready(sync_future) => ::std::task::Poll::Ready($sync_type {
                        $sync_future_field_name: sync_future,
                        $( $sync_field_name: impl_async_future_wrapper!(@to_sync_setter self, $field_name $(,  $copier )? $(, $action)?) ),*
                    }),
                }
            }
        }
    };
}

macro_rules! memoize_property_getter {
    (
        #[memoized_property($field_name:ident, $field_type:ty, Clone)]
        $method_vis:vis fn $method_name:ident(&self $(,)? $( $arg_name:ident: $arg_type: ty ),*) {
            $body:block
        }
    ) => {
        $method_vis fn $method_name(&self, $( $arg_name: $arg_type ),*) -> Result<$field_type> {
            let mut cache = self.$field_name.borrow_mut();
            if cache.is_none() {
                let resolved = {$body};
                *cache = Some(resolved);
            }
            Ok(cache.as_ref().unwrap().clone())
        }
    };
    (
        #[memoized_property($field_name:ident, $field_type:ty)]
        $method_vis:vis fn $method_name:ident(&self $(,)? $( $arg_name:ident: $arg_type: ty ),*) {
            $body:block
        }
    ) => {
        $method_vis fn $method_name(&self, $( $arg_name: $arg_type ),*) -> Result<$field_type> {
            let mut cache = self.$field_name.borrow_mut();
            if cache.is_none() {
                let resolved = {$body};
                *cache = Some(resolved);
            }
            Ok(cache.as_ref().unwrap())
        }
    };
}

pub(crate) use impl_async_future_wrapper;
pub(crate) use impl_serde_repr_str;
pub(crate) use memoize_property_getter;
