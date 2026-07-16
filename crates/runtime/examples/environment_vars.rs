//! Configuration from environment variables.
//!
//! Variable names are `PREFIX_FIELD`, uppercased: with `env_prefix = "MYAPP"`,
//! the field `port` reads `MYAPP_PORT`.
//!
//! The environment layer is only active when `env_prefix` is set. Without a
//! prefix a field named `path` would read the ambient `PATH`, so the derive
//! disables the layer rather than guess.
//!
//! ## Running this example
//!
//! ```bash
//! MYAPP_HOST=localhost MYAPP_PORT=8080 cargo run --example environment_vars
//!
//! # A flag you type still wins over the environment.
//! MYAPP_PORT=8080 cargo run --example environment_vars -- --port 9000
//! ```

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
    // `expect`/`?` would print the Debug representation, throwing away the
    // source-attributed message. Print the Display form instead.
    let cfg = Config::layered().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });

    println!("Host: {}", cfg.host);
    println!("Port: {}", cfg.port);
}
