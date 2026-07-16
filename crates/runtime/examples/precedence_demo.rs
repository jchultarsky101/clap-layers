//! The precedence rule, demonstrated end to end.
//!
//! Uses `layered_from` so every layer is injected explicitly and the example is
//! self-contained and deterministic — no exported variables required.
//!
//! ```bash
//! cargo run --example precedence_demo
//! ```

use clap::Parser;
use clap_layers::{Env, Layered};

#[derive(Parser, Layered, Debug)]
#[layered(file = "examples/config.toml", env_prefix = "MYAPP")]
struct Config {
    /// Port to listen on
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

fn main() -> Result<(), clap_layers::LayeredError> {
    // `examples/config.toml` sets `port = 8080`.
    let no_env = Env::empty();
    let with_env = Env::from_iter([("MYAPP_PORT".to_string(), "5000".to_string())]);

    println!("default only (no file would apply, no env, no flag)");
    println!("  nothing set anywhere         -> {}", 3000);

    let cfg = Config::layered_from(["demo"], &no_env)?;
    println!("file beats the default");
    println!("  config.toml port = 8080      -> {}", cfg.port);

    let cfg = Config::layered_from(["demo"], &with_env)?;
    println!("env beats the file");
    println!("  MYAPP_PORT=5000              -> {}", cfg.port);

    let cfg = Config::layered_from(["demo", "--port", "9000"], &with_env)?;
    println!("an explicit flag beats everything");
    println!("  --port 9000                  -> {}", cfg.port);

    // The subtle case this crate exists for: the user typed the default value.
    let cfg = Config::layered_from(["demo", "--port", "3000"], &with_env)?;
    println!("an explicit flag wins even when it equals the default");
    println!("  --port 3000 (== default)     -> {}", cfg.port);
    assert_eq!(
        cfg.port, 3000,
        "typing the default must still count as explicit"
    );

    Ok(())
}
