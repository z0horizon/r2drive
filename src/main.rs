use clap::Parser;
use r2drive::cli::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(err) = cli.execute().await {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}
