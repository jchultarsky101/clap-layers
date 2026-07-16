//! Dynamic values - fields that always come from environment or file.
//!
//! This example shows how to use `#[layered(no_cli)]` for fields that should
//! never be set via command-line arguments (e.g., auto-generated IDs, timestamps).
//!
//! ## Running this example
//!
//! ```bash
//! # No CLI args needed - instance_id will come from default
//! cargo run --example dynamic_values

use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
struct Config {
    /// Environment-specific identifier
    #[arg(long, default_value_t = String::from("production"))]
    environment: String,

    /// Instance ID (no_cli means can't be set via CLI)
    ///
    /// Since no_cli is set and there's a default value,
    /// the instance_id will use its default "generated" value.
    #[layered(no_cli)]
    #[arg(long, default_value_t = String::from("instance-001"))]
    instance_id: String,
}

fn main() {
    let cfg = Config::layered().expect("Failed to load configuration");

    println!("Configuration:");
    println!("  Environment: {}", cfg.environment);
    println!("  Instance ID: {}", cfg.instance_id);
}
