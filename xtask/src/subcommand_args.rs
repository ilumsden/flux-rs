use clap::Args;

#[derive(Args)]
pub struct BuildFixturesArgs {
    #[arg(short, long)]
    pub release: bool,
}

#[derive(Args)]
pub struct TestIntegrationArgs {
    #[arg(short, long)]
    pub release: bool,
    #[arg(long)]
    pub skip_teardown: bool,
}
