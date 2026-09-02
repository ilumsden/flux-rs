use flux_core::error::FluxError;
use flux_core::handle::{FluxHandle, HandleFlags};
use flux_core::rpc::{RpcFlags, RpcNodeId};

use anyhow::{Result, bail};
use clap::Parser;
use serde::Deserialize;

#[derive(Deserialize)]
struct HelloWorldPayload {
    greeting: String,
}

/// A simple exectuable to test RPCs to the test broker module
#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    hello_world_name: String,
    module_name: String,
    expected_service_name: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let handle = FluxHandle::new_from_str_uri("", HandleFlags::NONE)?;

    // for target_name in [&args.module_name, &args.expected_service_name] {
    for target_name in [&args.module_name] {
        let info_rpc_topic = format!("{}.info", target_name);

        println!("Sending RPC to '{info_rpc_topic}' target");

        // Send an initial RPC to <service_name>.info and validate the response
        let rpc_handle =
            handle.send_rpc(&info_rpc_topic, &[], RpcNodeId::Rank(0), RpcFlags::NONE)?;

        println!("Getting service name from RPC");
        match rpc_handle.get_string()? {
            Some(val) => {
                if val == &args.module_name {
                    println!("Got expected service name from broker module!");
                } else {
                    bail!("Got incorrect service name from broker module!");
                }
            }
            None => {
                bail!(
                    "Got an empty response from the broker module when checking for service name!"
                );
            }
        }

        println!("Sending Streaming RPC to '{info_rpc_topic}' target");

        // Scope used to control when the underlying flux_future_t
        // is released by the Rpc struct
        {
            // Send an initial RPC to <service_name>.info and validate the response
            let mut rpc_handle = handle.send_rpc(
                &info_rpc_topic,
                &[],
                RpcNodeId::Rank(0),
                RpcFlags::STREAMING,
            )?;

            println!("Getting service name from RPC");
            match rpc_handle.get_string()? {
                Some(val) => {
                    if val == &args.module_name {
                        println!("Got expected service name from broker module!");
                    } else {
                        bail!("Got incorrect service name from broker module!");
                    }
                }
                None => {
                    bail!(
                        "Got an empty response from the broker module when checking for service name!"
                    );
                }
            }

            rpc_handle.reset()?;

            println!("Checking for ENODATA to indicate end-of-stream");
            if let Err(flux_err) = rpc_handle.get() {
                match flux_err {
                    FluxError::EndOfStreamRpc => {
                        println!("Got expected ENODATA error to indicate end of streaming RPC!")
                    }
                    _ => bail!(
                        "Unexpected error occured while waiting for ENODATA: {}",
                        flux_err
                    ),
                }
            } else {
                bail!("Did not receive ENODATA to indicate end of streaming RPC.");
            }
        }

        let hello_world_rpc_topic = format!("{}.hello_world", target_name);

        println!("Sending RPC to '{hello_world_rpc_topic}' target");

        let rpc_hello_world_handle = handle.send_rpc_serializable(
            &hello_world_rpc_topic,
            &args.hello_world_name,
            RpcNodeId::Rank(0),
            RpcFlags::NONE,
        )?;

        match rpc_hello_world_handle.get_deserializable::<HelloWorldPayload>()? {
            Some(resp) => {
                let resp_msg = &resp.greeting;
                if *resp_msg
                    == format!(
                        "Hello, {}. You are running the flux-core-rs integration test broker module.",
                        args.hello_world_name
                    )
                {
                    println!(
                        "Got expected response from the `hello_world` topic from the broker module!"
                    );
                } else {
                    bail!("Got incorrect response from broker module: {}", resp_msg);
                }
            }
            None => {
                bail!("Got response with unexpected empty payload");
            }
        }
    }

    Ok(())
}
