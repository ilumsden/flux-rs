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

#[derive(Args)]
pub struct CoverageArgs {
    #[arg(short, long, default_value_t = false)]
    pub release: bool,
}

#[derive(Args)]
pub struct SetupPreCommitArgs {
    #[arg(long, default_value_t = false)]
    pub skip_pre_commit_install: bool,
}

#[derive(Args)]
pub struct UpdateDenyDbArgs {}
