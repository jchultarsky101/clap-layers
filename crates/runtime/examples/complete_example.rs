//! Every layer working together.
//!
//! ## Running this example
//!
//! ```bash
//! # Values come from examples/config.toml, falling back to defaults.
//! cargo run --example complete_example
//!
//! # The environment beats the file.
//! MYAPP_DATABASE_URL="postgres://prod:5432/prod" cargo run --example complete_example
//!
//! # A flag you type beats the environment and the file.
//! MYAPP_CACHE_TTL=60 cargo run --example complete_example -- --cache-ttl 120
//!
//! # Bad values are reported against the layer that supplied them, rather than
//! # being silently ignored:
//! #   invalid value 'banana' for 'cache_ttl' - from environment variable MYAPP_CACHE_TTL
//! MYAPP_CACHE_TTL=banana cargo run --example complete_example
//! ```

use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
#[command(
    version = "1.0.0",
    about = "Complete layered configuration example",
    long_about = None
)]
#[layered(file = "examples/config.toml", env_prefix = "MYAPP")]
struct Config {
    /// Database connection URL
    #[arg(long, default_value_t = String::from("sqlite://localhost:3000/db"))]
    database_url: String,

    /// Redis cache URL
    #[arg(long, default_value_t = String::from("redis://127.0.0.1:6379/0"))]
    redis_url: String,

    /// Cache time-to-live in seconds
    #[arg(long, default_value_t = 300)]
    cache_ttl: u64,

    /// Whether to enable debug mode
    #[arg(long, default_value_t = false)]
    debug: bool,
}

fn main() {
    // `expect`/`?` would print the Debug representation, throwing away the
    // source-attributed message. Print the Display form instead: every
    // LayeredError already names the layer and value at fault.
    let cfg = match Config::layered() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("configuration error: {e}");
            std::process::exit(1);
        }
    };

    println!("=== Application Configuration ===\n");

    println!("Database: {}", cfg.database_url);
    println!("Redis:    {}", cfg.redis_url);
    println!("Cache TTL: {}s ({}m)", cfg.cache_ttl, cfg.cache_ttl / 60);
    println!("Debug:     {}", cfg.debug);
}
