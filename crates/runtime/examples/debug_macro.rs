//! Debug macro expansion - demonstrates what the Layered derive generates.
//!
//! This example shows how to inspect what the #[derive(Layered)] macro produces.

use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
struct Config {
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

fn main() {
    // This example is useful for debugging - run with cargo expand
    // to see the generated code:
    //
    // ```bash
    // cargo install cargo-expand
    // cargo expand --example debug_macro
    // ```
    
    let cfg = Config::layered().expect("Failed to load config");
    println!("Config loaded: {:?}", cfg);
}
