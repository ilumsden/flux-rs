use flux_core::handle::{FluxHandle, HandleFlags};
use flux_core::rpc::{RpcFlags, RpcNodeId};

use anyhow::{Result, bail};
use clap::Parser;

/// A simple exectuable to test RPCs to the test broker module
#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    name: String,
    expected_service_name: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let handle = FluxHandle::new_from_str_uri("", HandleFlags::NONE)?;

    let info_rpc_topic = format!("{}.info", args.expected_service_name);

    // Send an initial RPC to <service_name>.info and validate the response
    let rpc_handle = handle.send_rpc(&info_rpc_topic, &[], RpcNodeId::Rank(0), RpcFlags::NONE)?;

    match rpc_handle.get_json()? {
        Some(val) => {
            if let Some(val_str) = val.as_str()
                && val_str == &args.expected_service_name
            {
                println!("Got expected service name from broker module!");
            } else {
                bail!("Got incorrect service name from broker module!");
            }
        }
        None => {
            bail!("Got an empty response from the broker module when checking for service name!");
        }
    }

    let hello_world_rpc_topic = format!("{}.hello_world", args.expected_service_name);

    let rpc_hello_world_handle = handle.send_rpc_serializable(
        &hello_world_rpc_topic,
        &args.name,
        RpcNodeId::Rank(0),
        RpcFlags::NONE,
    )?;

    match rpc_hello_world_handle.get_deserializable::<String>()? {
        Some(resp_msg) => {
            if resp_msg
                == format!(
                    "Hello, {}. You are running the flux-core-rs integration test broker module.",
                    args.name
                )
            {
                println!(
                    "Got expected response from the `hellow_world` topic from the broker module!"
                );
            } else {
                bail!("Got incorrect response from broker module: {}", resp_msg);
            }
        }
        None => {
            bail!("Got response with unexpected empty payload");
        }
    }

    Ok(())
}
