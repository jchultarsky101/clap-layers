//! CLI-only configuration - simplest possible setup.
//!
//! This example shows the most basic use case: only command-line arguments
//! with default values. No environment variables or config files are used.

use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
struct Config {
    /// Port to listen on
    #[arg(long, short, default_value_t = 3000)]
    port: u16,

    /// Output verbosity
    #[arg(long, default_value_t = false)]
    verbose: bool,
}

fn main() {
    // `expect`/`?` would print the Debug representation, throwing away the
    // source-attributed message. Print the Display form instead.
    let cfg = Config::layered().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });
    println!("Configuration loaded:");
    println!("  Port: {}", cfg.port);
    println!("  Verbose: {}", cfg.verbose);

    // Use the configuration...
    if cfg.verbose {
        println!("Running in verbose mode on port {}", cfg.port);
    }
}
