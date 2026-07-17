# clap-layers

[![Crates.io](https://img.shields.io/crates/v/clap-layers.svg)](https://crates.io/crates/clap-layers)
[![Docs.rs](https://docs.rs/clap-layers/badge.svg)](https://docs.rs/clap-layers)
[![CI](https://github.com/jchultarsky/clap-layers/actions/workflows/ci.yml/badge.svg)](https://github.com/jchultarsky/clap-layers/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](#msrv)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> **Early release.** The correctness bar below is implemented and covered by the test suite.
> The API is not yet settled: until `1.0`, a breaking change bumps the minor version, so pin
> `0.1` if that matters to you.

One derive macro that gives any [clap](https://crates.io/crates/clap) application **correct** layered configuration — an explicit **CLI flag** beats an **env var** beats a **config file** beats a **built-in default** — from a single struct definition. `--help` still shows real defaults, and errors name the source file and line.

## Install

```sh
cargo add clap-layers clap --features clap/derive
```

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

```rust,no_run
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

    /// Config/env only - never exposed as a CLI flag.
    /// `#[arg(skip)]` is what actually keeps clap from defining a flag;
    /// the derive rejects `no_cli` without it rather than silently exposing the field.
    #[layered(no_cli)]
    #[arg(skip = 5u32)]
    retry_budget: u32,
}

fn main() {
    // `layered()` handles CLI errors and `--help` itself, exactly as clap's
    // `parse()` does. Print the Display form of anything else: `?` in `main`
    // prints the *Debug* representation, which throws the message away.
    let cfg = Config::layered().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });

    println!("{cfg:?}"); // flag > env > file > default
}
```

### Testing your configuration

`layered()` reads the real process arguments and environment. `layered_from` takes both
explicitly, so tests are hermetic and run in parallel without touching process globals:

```rust
# use clap::Parser;
# use clap_layers::{Env, Layered};
# #[derive(Parser, Layered, Debug)]
# #[layered(env_prefix = "MYAPP")]
# struct Config {
#     #[arg(long, default_value_t = 3000)]
#     port: u16,
# }
let env = Env::from_iter([("MYAPP_PORT", "8080")]);

// The environment beats the default...
assert_eq!(Config::layered_from(["myapp"], &env)?.port, 8080);

// ...but an explicitly typed flag beats the environment, even when the
// value typed happens to equal the default.
assert_eq!(Config::layered_from(["myapp", "--port", "3000"], &env)?.port, 3000);
# Ok::<(), clap_layers::LayeredError>(())
```

### Environment variables

The environment layer is **only active when `env_prefix` is set**: without a prefix a field
named `path` would read the ambient `PATH`, so the derive disables the layer rather than
guess. Names are `PREFIX_FIELD`, uppercased — `env_prefix = "MYAPP"` maps `db_password` to
`MYAPP_DB_PASSWORD`.

Values are decoded with `serde`, so any `Deserialize` field type works — `u16`, `bool`,
`String`, `Vec<T>`, `Option<T>`, and `#[derive(Deserialize)]` enums. Environment values are
read as TOML values, so `MYAPP_TAGS='["a", "b"]'` populates a `Vec<String>`; anything that
is not valid TOML is taken as a plain string, so `MYAPP_HOST=localhost` needs no quoting.

### Errors name their source

A value that cannot be decoded is a hard error attributed to the layer that supplied it —
never a silent fall back to the default:

```text
invalid value 'not-a-number' for 'port' — from myapp.toml, line 12 (invalid type: string, expected u16)
invalid value 'banana' for 'port' — from environment variable MYAPP_PORT (invalid type: string, expected u16)
```

A *missing* config file is not an error; the layer is skipped. A file that exists but cannot
be read or parsed is reported, with a line and column.

## Design goals (the correctness bar)

1. **Explicit-vs-default detection** — a config value beats a default but loses to an explicitly passed flag (via clap's `ValueSource`). clap's own `#[arg(env = "...")]` composes: a value clap read from the environment also counts as explicit, and keeps beating the file layer.
2. **`--help` survives** — fields keep native `default_value_t`, so help shows `[default: 3000]`, not `None`.
3. **One struct, no duplication** — a single struct drives clap parsing, `serde` deserialization, and env reading. Any partial/shadow struct is generated internally.
4. **Source-attributed errors** — `invalid value 'foo' for 'port' — from config.toml, line 12`.
5. **Field-level control** — `no_cli` / `no_file` / `no_env` markers; per-field merge strategies land in `v0.2`. A malformed or misspelled marker is a compile error, never a silently ignored attribute.

## Comparison

| Crate                | clap-aware | Correct explicit-vs-default | Source-attributed errors | Maintained |
| -------------------- | ---------- | --------------------------- | ------------------------ | ---------- |
| **clap-layers**      | ✅         | ✅                          | ✅                       | ✅         |
| twelf                | ✅         | partial                     | ❌                       | ⚠️ stale   |
| confique             | ❌         | n/a                         | ✅                       | ✅         |
| figment              | ❌         | n/a                         | ✅                       | ✅         |
| clap-config-file     | ✅         | partial                     | partial                  | ✅         |

*The `clap-layers` row reflects behaviour covered by the test suite. Other rows are a
point-in-time reading of each crate's documentation — check their docs for current specifics.*

## Known limitations

- A field with **no default and no `Option<T>`** cannot be filled from the config file
  or the environment. clap enforces required-ness while parsing, before any lower layer
  is consulted, so give such a field a `default_value_t` or make it `Option<T>`.
- Config-file keys that match no field are **ignored rather than rejected**, so a typo in
  a config file silently does nothing. A strict mode is a candidate for `v0.2`.
- Sequences from the environment must be written as TOML arrays — `MYAPP_TAGS='["a","b"]'`.
  clap's `value_delimiter` applies to the command line only.
- `#[layered(no_cli)]` requires `#[arg(skip)]` on the same field, because a separate derive
  cannot remove an argument clap has already defined. The derive rejects the pair rather
  than letting the field stay exposed.

## What this doesn't do yet

- **Subcommands** (enum ↔ struct mapping) are **out of scope for `v0.1`** and planned for a later release. A struct that *has* a subcommand still works: the field is passed through untouched while its siblings layer normally.
- Only **TOML** is built in initially; other `serde::Deserialize` formats are pluggable.
- Per-field merge strategies, `--dump-config`, and config-file discovery land in `v0.2`.

## Roadmap

- **v0.1** *(implemented)* — derive macro, `flag > env > file > default` precedence, TOML source, env source with prefix, source-attributed errors, `no_cli` / `no_file` / `no_env` markers, precedence-matrix test suite.
- **v0.2** — per-field merge strategies, `--dump-config`, file discovery + `--isolated`, pluggable formats.
- **v0.3** — subcommand support.

## MSRV

Minimum supported Rust version: **1.85** (required by edition 2024).

## Design & contributing

- **[Design & Rationale](https://github.com/jchultarsky/clap-layers/blob/main/docs/DESIGN.md)** —
  why this crate exists, the correctness requirements it must meet, the target API, and how the
  derive works internally. Start here to understand the project's intent.
- **[Contributing guide](CONTRIBUTING.md)** — how to build, test, and open a PR.
- **[Changelog](CHANGELOG.md)** — notable changes per release.

Contributions are welcome — especially tests and error-message improvements.

## License

Licensed under the [MIT License](LICENSE).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you shall be licensed as above, without any
additional terms or conditions.
