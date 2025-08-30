use clap::{Parser, Subcommand};

mod library_commands;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan library. Default: scan all accounts. Optional: use 'account' flag to specify a single account.
    Scan(Scan),
}

#[derive(Parser, Debug)]
#[command(about = "Scan library. Default: scan all accounts. Optional: use 'account' flag to specify a single account.")]
struct Scan {
    /// Optional: user ID or nicknames of accounts to scan.
    #[arg(required = false)]
    account_names: Vec<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Scan(scan_args) => {
            library_commands::import_account_async(&scan_args.account_names).await;
        }
    }
}
