//! Using environment variables for configuration.
//!
//! This example demonstrates how to load configuration from environment variables.
//!
//! ## Running this example
//!
//! ```bash
//! # Set individual environment variables (note: field names are lowercase)
//! export MYAPP_HOST=localhost
//! export MYAPP_PORT=8080
//! cargo run --example environment_vars
//!
//! # Or set them inline
//! MYAPP_HOST=localhost MYAPP_PORT=8080 cargo run --example environment_vars
//! ```
//!
//! **Note:** Environment variable names are `{PREFIX}_{FIELD_NAME}` where `FIELD_NAME` is lowercase.
//! For field `port`, the env var is `MYAPP_port`. For `host`, it's `MYAPP_host`.

use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
#[layered(env_prefix = "MYAPP")]
struct Config {
    /// Host to bind to
    #[arg(long, default_value_t = String::from("127.0.0.1"))]
    host: String,

    /// Port to listen on
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

fn main() {
    let cfg = Config::layered().expect("Failed to load config");

    println!("Server configuration:");
    println!("  Host: {}", cfg.host);
    println!("  Port: {}", cfg.port);
}
