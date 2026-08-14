use std::cell::RefCell;
use std::ffi::CStr;

use flux_core::error::{FluxError, Result};
use flux_core::flux_log_error;
use flux_core::handle::FluxHandle;
use flux_core::module::{create_module_entrypoint, set_global_panicking_allocator};
use flux_core::msg::{Message, MessageRolemask, MessageType};
use flux_core::msg_handler::{MsgHandler, MsgHandlerSpec, add_handler_vec};
use flux_core::reactor::ReactorFlags;
use flux_core::request::Request;
use flux_core::service::register_service;

use clap::Parser;

set_global_panicking_allocator!();

thread_local! {
    static STREAMING_REQUESTS: RefCell<Vec<Message>> = RefCell::new(Vec::new());
}

fn info(handle: FluxHandle, _msg_handler: MsgHandler, msg: Message) {
    let service_name = match handle.get_aux_raw("flux::name") {
        Ok(c_str_ptr) => unsafe { CStr::from_ptr(c_str_ptr as *const u8) },
        Err(e) => {
            flux_log_error!(handle, "Error getting service name: {}", e);
            c""
        }
    };
    let request = Request::from(msg);
    if let Err(e) = handle.respond_string(&request, Some(&service_name.to_string_lossy())) {
        flux_log_error!(handle, "Error responding to the info request: {}", e);
    }
    if request.is_streaming() {
        STREAMING_REQUESTS.with(|msg_vec| msg_vec.borrow_mut().push(request.clone()));
    }
}

fn hello_world(handle: FluxHandle, _msg_handler: MsgHandler, msg: Message) {
    let request = Request::from(msg);
    let user_name: String = match request.decode_deserializable() {
        Ok(dec_resp) => match dec_resp.payload {
            Some(n) => n,
            None => "UNKNOWN".to_string(),
        },
        Err(e) => {
            flux_log_error!(handle, "Failed to decode payload: {}", e);
            return;
        }
    };
    let response_string = format!(
        "Hello, {}. You are running the flux-core-rs integration test broker module.",
        user_name
    );
    if let Err(e) = handle.respond_string(&request, Some(&response_string)) {
        flux_log_error!(handle, "Failed to respond to the RPC request: {}", e);
    }
}

/// A simple broker module serving as an integration test
#[derive(Parser, Debug)]
#[command(version, about, long_about=None, no_binary_name = true)]
struct Args {
    /// Name of the service to spawn
    #[arg(short, long)]
    service: Option<String>,

    /// If provided, trigger a failure in mod_main to abort
    #[arg(short, long, default_value_t = false)]
    init_failure: bool,
}

fn module_main(handle: FluxHandle, args: Vec<String>) -> Result<()> {
    let parsed_args: Args = match Args::try_parse_from(args) {
        Ok(a) => a,
        Err(e) => {
            return Err(FluxError::Logic(format!(
                "Failed to parse module arguments:\n{}",
                e
            )));
        }
    };
    if let Some(service_name) = parsed_args.service {
        register_service(&handle, &service_name)?;
    }
    if parsed_args.init_failure {
        return Err(FluxError::Logic(
            "Aborting during init per test request".to_string(),
        ));
    }

    let handler_vec = vec![
        MsgHandlerSpec::new(MessageType::REQUEST, "info", info, MessageRolemask::NONE),
        MsgHandlerSpec::new(
            MessageType::REQUEST,
            "hello_world",
            hello_world,
            MessageRolemask::NONE,
        ),
    ];

    add_handler_vec(&handle, handler_vec)?;

    if let Err(e) = handle.get_reactor()?.run(ReactorFlags::NONE) {
        flux_log_error!(handle, "Error occured in Reactor::run: {}", e);
    }

    STREAMING_REQUESTS.with(|msg_vec| {
        let streaming_msgs = msg_vec.take();
        for msg in streaming_msgs {
            let request = Request::from(msg);
            if let Err(e) = handle.respond_raw_error(&request, libc::ENODATA, None) {
                flux_log_error!(
                    handle,
                    "Failed to send end-of-stream response to RPC via flux_respond_error: {}",
                    e
                );
            }
            // The flux_msg_t pointer is destroyed (via flux_msg_decref) automatically by
            // the drop method on Message and Request.
        }
    });

    Ok(())
}

create_module_entrypoint!(self::module_main);
