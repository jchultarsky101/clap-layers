//! Quick start guide for clap-layers
//!
//! This example demonstrates the most common use case: a CLI app with
//! layered configuration from multiple sources.
//!
//! ## Quick Start
//!
//! Run this example:
//! ```bash
//! cargo run --example quickstart
//! ```
//!
//! Try different configurations:
//! ```bash
//! # Use all defaults (port=3000, verbose=0, debug=false)
//! cargo run --example quickstart
//!
//! # Override via CLI flag (highest priority)
//! cargo run --example quickstart -- --port 9000 --verbose
//!
//! # Override via environment variable (note: field names are lowercase)
//! MYAPP_port=8080 MYAPP_debug=true cargo run --example quickstart
//!
//! # Create a file named config.toml in runtime/examples/ with:
//! #   port = 5000
//! #   verbose = 1
//! # And run: cargo run --example quickstart
//! ```

use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
#[command(name = "myapp")]
#[command(version = "1.0")]
#[command(about = "A simple CLI app with layered configuration", long_about = None)]
#[layered(file = "config.toml", env_prefix = "MYAPP")]
struct Config {
    /// Port to listen on
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// Verbosity level (repeat for more verbosity)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Enable debug mode
    #[arg(long, default_value_t = false)]
    debug: bool,
}

fn main() {
    let cfg = Config::layered().expect("Failed to load configuration");

    println!("=== Application Started ===\n");
    println!("Configuration:");
    println!("  Port: {}", cfg.port);
    println!("  Verbose level: {}", cfg.verbose);
    println!("  Debug mode: {}", cfg.debug);

    // Your application logic here...
    if cfg.debug {
        eprintln!("\n[DEBUG] Configuration loaded successfully");
    }

    println!("\nRunning with the configuration above...");
}
