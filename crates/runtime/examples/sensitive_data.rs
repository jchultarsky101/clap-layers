//! Handling sensitive data - keeping passwords out of environment and config files.
//!
//! This example demonstrates how to use `#[layered(no_env, no_file)]` to ensure
//! that sensitive fields are not read from environment variables or config files.
//!
//! ## Running this example
//!
//! ```bash
//! # Provide password via CLI (required since no default)
//! cargo run --example sensitive_data -- --db-password "secret123"
//!
//! # Environment variable is ignored due to no_env attribute
//! DB_PASSWORD=exposed cargo run --example sensitive_data -- --db-password "secret123"
//! ```

use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
struct Config {
    /// Database username (can be in env/file)
    #[arg(long, default_value_t = String::from("admin"))]
    db_user: String,

    /// Database password
    ///
    /// The `no_env` and `no_file` attributes prevent this field from being
    /// set via environment variables or config files. Since there's no default,
    /// it must be provided via CLI.
    #[layered(no_env, no_file)]
    #[arg(long)]
    db_password: String,
}

fn main() {
    let cfg = Config::layered().expect("Failed to load configuration");

    println!("Configuration:");
    println!("  DB User: {}", cfg.db_user);
    
    // Only show first few characters for security
    let display_pwd = &cfg.db_password.chars().take(3).collect::<String>();
    println!("  DB Password: ***{}***", display_pwd);
}
