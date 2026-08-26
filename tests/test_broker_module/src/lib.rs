use std::ffi::CStr;

use flux_core::error::{FluxError, Result};
use flux_core::handle::FluxHandle;
use flux_core::module::{create_module_entrypoint, set_global_panicking_allocator};
use flux_core::msg::{Message, MessageRolemask, MessageType};
use flux_core::msg_handler::{BorrowedMsgHandler, MsgHandlerSpec, add_handler_vec};
use flux_core::reactor::ReactorFlags;
use flux_core::request::Request;
use flux_core::service::register_service;
use flux_core::{flux_log_error, flux_log_info};

use clap::Parser;
use serde::Serialize;

set_global_panicking_allocator!();

fn info(handle: FluxHandle, _msg_handler: BorrowedMsgHandler<'_>, msg: Message) {
    flux_log_info!(handle, "INFO - Getting service name");
    let service_name = match handle.get_aux_raw("flux::name") {
        Ok(c_str_ptr) => unsafe { CStr::from_ptr(c_str_ptr as *const u8) },
        Err(e) => {
            flux_log_error!(handle, "Error getting service name: {}", e);
            c""
        }
    };
    let request = Request::from(msg);
    flux_log_info!(
        handle,
        "INFO - Responding with service name '{}'",
        service_name.to_string_lossy()
    );
    if let Err(e) = handle.respond_string(&request, Some(&service_name.to_string_lossy())) {
        flux_log_error!(handle, "Error responding to the info request: {}", e);
    }
    if request.is_streaming() {
        flux_log_info!(
            handle,
            "INFO - Streaming request detected. Sending ENODATA."
        );
        if let Err(e) = handle.respond_raw_error(&request, libc::ENODATA, None) {
            flux_log_error!(
                handle,
                "Failed to send end-of-stream response to RPC via flux_respond_error: {}",
                e
            );
        }
    }
}

#[derive(Serialize)]
struct HelloWorldPayload {
    greeting: String,
}

fn hello_world(handle: FluxHandle, _msg_handler: BorrowedMsgHandler<'_>, msg: Message) {
    flux_log_info!(handle, "HELLO_WORLD - Decoding request");
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
    flux_log_info!(
        handle,
        "HELLO_WORLD - Got user name '{}' from request",
        user_name
    );
    let response_string = format!(
        "Hello, {}. You are running the flux-core-rs integration test broker module.",
        user_name
    );
    let payload = HelloWorldPayload {
        greeting: response_string,
    };
    flux_log_info!(handle, "HELLO_WORLD - Responding to request");
    if let Err(e) = handle.respond_serializable(&request, Some(&payload)) {
        flux_log_error!(
            handle,
            "HELLO_WORLD - Failed to respond to the RPC request: {}",
            e
        );
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

    /// If provided, panic the broker module to ensure the panic won't kill the broker
    #[arg(long, default_value_t = false)]
    init_panic: bool,
}

fn module_main(handle: FluxHandle, args: Vec<String>) -> Result<()> {
    flux_log_info!(handle, "Parsing command-line arguments");
    let parsed_args: Args = match Args::try_parse_from(args) {
        Ok(a) => a,
        Err(e) => {
            return Err(FluxError::Logic(format!(
                "Failed to parse module arguments:\n{}",
                e
            )));
        }
    };
    flux_log_info!(handle, "Checking if service name was provided");
    if let Some(service_name) = parsed_args.service {
        flux_log_info!(
            handle,
            "Registering module under service name '{}'",
            service_name
        );
        register_service(&handle, &service_name)?;
    }
    if parsed_args.init_failure {
        flux_log_info!(
            handle,
            "Triggering failure because the user requested to do so via command-line arguments"
        );
        return Err(FluxError::Logic(
            "Aborting during init per test request".to_string(),
        ));
    }
    if parsed_args.init_panic {
        panic!("Panicking during init per test request");
    }

    flux_log_info!(
        handle,
        "Creating Vec of MsgHandlerSpecs for topics/callbacks"
    );
    let service_name = unsafe { CStr::from_ptr(handle.get_aux_raw("flux::name")? as *const u8) };
    let service_name_ru = service_name.to_string_lossy();
    let handler_vec = vec![
        MsgHandlerSpec::new(MessageType::REQUEST, "info", info, MessageRolemask::NONE),
        MsgHandlerSpec::new(
            MessageType::REQUEST,
            "hello_world",
            hello_world,
            MessageRolemask::NONE,
        ),
    ];
    flux_log_info!(
        handle,
        "Will try to register {} MsgHandlerSpecs",
        handler_vec.len()
    );

    // Store the handlers here so that Drop will automatically stop them at the end of this function
    flux_log_info!(
        handle,
        "Registering specs using FluxHandle and add_handler_vec"
    );
    let handlers = add_handler_vec(&handle, handler_vec, Some(&service_name_ru))?;
    flux_log_info!(
        handle,
        "Successfully obtained {} handlers from add_handler_vec",
        handlers.len()
    );

    flux_log_info!(handle, "Running Reactor associated with FluxHandle");
    if let Err(e) = handle.get_reactor()?.run(ReactorFlags::NONE) {
        flux_log_error!(handle, "Error occured in Reactor::run: {}", e);
    }

    flux_log_info!(handle, "Returning Ok from module_main");

    Ok(())
}

create_module_entrypoint!(self::module_main);
