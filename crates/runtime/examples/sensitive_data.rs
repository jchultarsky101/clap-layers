//! Keeping secrets out of the environment and config file.
//!
//! `#[layered(no_env, no_file)]` removes those two layers for a single field,
//! so a password can only ever arrive from the command line. The other fields
//! still layer normally.
//!
//! ## Running this example
//!
//! ```bash
//! # The password must come from the CLI.
//! cargo run --example sensitive_data -- --db-password "secret123"
//!
//! # MYAPP_DB_USER is honoured; MYAPP_DB_PASSWORD is ignored, because
//! # `db_password` opts out of the environment layer.
//! MYAPP_DB_USER=readonly MYAPP_DB_PASSWORD=exposed \
//!   cargo run --example sensitive_data -- --db-password "secret123"
//! ```

use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
#[layered(file = "examples/config.toml", env_prefix = "MYAPP")]
struct Config {
    /// Database username; may come from any layer.
    #[arg(long, default_value_t = String::from("admin"))]
    db_user: String,

    /// Database password.
    ///
    /// `no_env` and `no_file` mean neither `MYAPP_DB_PASSWORD` nor a
    /// `db_password` key in the config file can populate this field.
    #[layered(no_env, no_file)]
    #[arg(long)]
    db_password: String,
}

fn main() {
    // `expect`/`?` would print the Debug representation, throwing away the
    // source-attributed message. Print the Display form instead.
    let cfg = Config::layered().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });

    println!("DB user:     {}", cfg.db_user);
    // Never print a secret in full.
    println!(
        "DB password: {}***",
        &cfg.db_password[..1.min(cfg.db_password.len())]
    );
}
