use clap::Args;

#[derive(Args)]
pub struct BuildFixturesArgs {
    #[arg(short, long)]
    pub release: bool,
}

#[derive(Args)]
pub struct TestBrokerRpcArgs {
    #[arg(short, long)]
    pub release: bool,

    #[arg(short, long, default_value = "rust-test")]
    pub service_name: String,

    #[arg(short, long, default_value = "rust_broker_module")]
    pub broker_module_name: String,

    #[arg(short, long, default_value = "flux_user")]
    pub uname: String,
}
