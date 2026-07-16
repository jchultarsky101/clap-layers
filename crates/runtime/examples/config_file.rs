//! Configuration from a TOML file.
//!
//! Paths in `#[layered(file = "...")]` are resolved relative to the process's
//! working directory, which is the workspace root under `cargo run`.
//!
//! A missing config file is not an error — the layer is simply skipped. A file
//! that exists but is unreadable or malformed *is* an error, and names the line.
//!
//! ## Running this example
//!
//! ```bash
//! cargo run --example config_file
//! ```

use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
#[layered(file = "examples/config.toml")]
struct Config {
    /// Host to bind to
    #[arg(long, default_value_t = String::from("127.0.0.1"))]
    host: String,

    /// Port to listen on
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

fn main() {
    // `expect`/`?` would print the Debug representation, throwing away the
    // source-attributed message. Print the Display form instead.
    let cfg = Config::layered().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });

    println!("Host: {} (config.toml sets 0.0.0.0)", cfg.host);
    println!("Port: {} (config.toml sets 8080)", cfg.port);
}
