# clap-layers

[![Crates.io](https://img.shields.io/crates/v/clap-layers.svg)](https://crates.io/crates/clap-layers)
[![Docs.rs](https://docs.rs/clap-layers/badge.svg)](https://docs.rs/clap-layers)
[![CI](https://github.com/jchultarsky101/clap-layers/actions/workflows/ci.yml/badge.svg)](https://github.com/jchultarsky101/clap-layers/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> ⚠️ **Pre-release / work in progress.** The API described below is the design target for `v0.1.0` and is not fully implemented yet. Expect breaking changes before `1.0`.

One derive macro that gives any [clap](https://crates.io/crates/clap) application **correct** layered configuration — an explicit **CLI flag** beats an **env var** beats a **config file** beats a **built-in default** — from a single struct definition. `--help` still shows real defaults, and errors name the source file and line.

## Why

clap deliberately keeps layered configuration out of core, inviting the ecosystem to experiment. Several crates have tried; each stumbles on at least one correctness trap. `clap-layers` aims to win on **correctness, error quality, and documentation** rather than novelty.

## Precedence

Highest precedence wins. A value set at a higher layer overrides the same value at every lower layer.

| Priority | Layer            | Example                          |
| -------- | ---------------- | -------------------------------- |
| 1        | Explicit CLI arg | `--port 8080`                    |
| 2        | Environment var  | `MYAPP_PORT=8080`                |
| 3        | Config file      | `port = 8080` in `myapp.toml`    |
| 4        | Built-in default | `#[arg(default_value_t = 3000)]` |

The key subtlety: a config-file value overrides a clap **default**, but loses to a flag the user **explicitly passed** — even when the passed value equals the default.

## Example

```rust
use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
#[command(version, about)]
#[layered(file = "myapp.toml", env_prefix = "MYAPP")]
struct Config {
    /// Port to listen on
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// Verbosity
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// Config/env only — never exposed as a CLI flag
    #[layered(no_cli)]
    retry_budget: u32,
}

fn main() -> anyhow::Result<()> {
    let cfg = Config::layered()?; // flag > env > file > default
    println!("{cfg:?}");
    Ok(())
}
```

## Design goals (the correctness bar)

1. **Explicit-vs-default detection** — a config value beats a default but loses to an explicitly passed flag (via clap's `ValueSource`).
2. **`--help` survives** — fields keep native `default_value_t`, so help shows `[default: 3000]`, not `None`.
3. **One struct, no duplication** — a single struct drives clap parsing, `serde` deserialization, and env reading. Any partial/shadow struct is generated internally.
4. **Source-attributed errors** — `invalid value 'foo' for 'port' — from config.toml, line 12`.
5. **Field-level control** — per-field merge strategies and `no_cli` / `no_file` / `no_env` markers.

## Comparison

| Crate                | clap-aware | Correct explicit-vs-default | Source-attributed errors | Maintained |
| -------------------- | ---------- | --------------------------- | ------------------------ | ---------- |
| **clap-layers**      | ✅         | ✅ (goal)                   | ✅ (goal)                | ✅         |
| twelf                | ✅         | partial                     | ❌                       | ⚠️ stale   |
| confique             | ❌         | n/a                         | ✅                       | ✅         |
| figment              | ❌         | n/a                         | ✅                       | ✅         |
| clap-config-file     | ✅         | partial                     | partial                  | ✅         |

*Comparison reflects the design target; see each crate's docs for current specifics.*

## What this doesn't do yet

- **Subcommands** (enum ↔ struct mapping) are **out of scope for `v0.1`** and planned for a later release.
- Only **TOML** is built in initially; other `serde::Deserialize` formats are pluggable.
- Per-field merge strategies, `--dump-config`, and config-file discovery land in `v0.2`.

## Roadmap

- **v0.1** — derive macro, `flag > env > file > default` precedence, TOML source, env source with prefix, source-attributed errors, `no_cli` / `no_file` markers, precedence-matrix test suite.
- **v0.2** — per-field merge strategies, `--dump-config`, file discovery + `--isolated`, pluggable formats.
- **v0.3** — subcommand support.

## MSRV

Minimum supported Rust version: **1.85** (required by edition 2024).

## License

Licensed under the [MIT License](LICENSE).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you shall be licensed as above, without any
additional terms or conditions.
