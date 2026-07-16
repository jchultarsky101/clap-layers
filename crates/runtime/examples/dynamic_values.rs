//! Fields that must never be a CLI flag.
//!
//! `#[layered(no_cli)]` hides a field from the command line entirely. Because
//! clap builds the flag list, the field also needs `#[arg(skip)]` so clap
//! leaves it alone — the derive enforces that pairing rather than letting the
//! field be silently exposed.
//!
//! `#[arg(skip = expr)]` supplies the built-in default, standing in for the
//! `default_value_t` a normal flag would use.
//!
//! ## Running this example
//!
//! ```bash
//! # instance_id falls back to its built-in default.
//! cargo run --example dynamic_values
//!
//! # ...and can still be set from the environment.
//! MYAPP_INSTANCE_ID=instance-042 cargo run --example dynamic_values
//!
//! # ...but never from the command line: this is an unknown-argument error.
//! cargo run --example dynamic_values -- --instance-id nope
//! ```

use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
#[layered(env_prefix = "MYAPP")]
struct Config {
    /// Deployment environment; a normal flag.
    #[arg(long, default_value_t = String::from("production"))]
    environment: String,

    /// Instance ID: environment or file only, never a flag.
    #[layered(no_cli)]
    #[arg(skip = String::from("instance-001"))]
    instance_id: String,
}

fn main() {
    // `expect`/`?` would print the Debug representation, throwing away the
    // source-attributed message. Print the Display form instead.
    let cfg = Config::layered().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });

    println!("Environment: {}", cfg.environment);
    println!("Instance ID: {}", cfg.instance_id);
}
