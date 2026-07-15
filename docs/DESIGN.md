# Design & Rationale

This document explains **why `clap-layers` exists, what it must get right, and how it is meant
to work internally**. It's aimed at contributors and anyone curious about the design decisions.
For usage, see the [README](../README.md); for the working agreement (build/test/lint loop,
coverage, API rules), see [CLAUDE.md](../CLAUDE.md).

## Motivation

[clap](https://crates.io/crates/clap) is the de-facto CLI parser for Rust, but it deliberately
does **not** handle layered configuration — merging values from CLI flags, environment
variables, and a config file with sensible precedence. clap's maintainer keeps this out of core
on purpose and has invited the ecosystem to build it on top and see what works.

The demand has been real and unresolved for years: a long-running clap discussion collected many
subscribers asking for exactly this, with no single "blessed" solution emerging. Several crates
have attempted it, and each stumbles on at least one of the correctness traps below. clap even
merged a documentation example wiring config manually — a docs page standing in for a crate that
should exist.

So the opportunity here is **not novelty — it's quality**. This crate wins by being *correct*,
by producing *excellent errors*, and by being *well documented*, not by inventing a new concept.
That framing drives every requirement that follows.

## What it must get right (the five requirements)

These are the acceptance criteria. They're the reasons the crate exists; the test suite must
prove each one, and no change may regress them.

1. **Explicit-vs-default detection.** A value in the config file must override a clap *default*,
   but must lose to a flag the user *explicitly passed on the command line* — even when the
   passed value happens to equal the default. This is the core correctness claim. It relies on
   clap's `ArgMatches::value_source()` to tell "the user typed this" apart from "this is the
   default," and that distinction must be plumbed through the merge, hidden behind the derive.

2. **`--help` must keep showing real defaults.** A tempting-but-wrong implementation wraps every
   field in `Option<T>` to detect whether it was set. That breaks `--help`, which then shows
   every field as optional with no default. Fields must keep their native clap
   `default_value_t`, so help still prints `[default: 3000]`. Presence is detected via
   `value_source()`, never by making fields optional.

3. **One struct, no duplication.** The user writes a **single** struct. Its fields and
   doc-comments drive clap parsing, `serde` deserialization, and environment reading all at once.
   The user must never have to hand-maintain a parallel "partial" or shadow struct. Generating
   such a struct *inside* the macro is fine and expected — it just has to be invisible.

4. **Source-attributed errors.** When a value is invalid, the error names the layer it came from,
   e.g. `invalid value 'foo' for 'port' — from config.toml, line 12`. A merge that collapses all
   sources first and only then reports a generic failure — leaving the user unable to tell which
   file or variable was at fault — is exactly the failure mode we're reacting against.

5. **Field-level control.** Merge behavior is often field-specific, so it can't be a single
   top-level policy. Support per-field merge strategies (e.g. replace vs. append) and per-field
   layer markers (`no_cli` / `no_file` / `no_env`) for values that shouldn't exist in every
   layer. This directly answers long-standing feedback that a one-size-fits-all merge is wrong.

## Additional requirements

Beyond the five, these round out a credible v1-track design:

- **`--dump-config`**: print the effective, merged configuration. Repeatedly requested by users.
- **Config-file discovery**: an explicit path flag, plus implicit discovery (walk up from CWD,
  and XDG locations), plus an `--isolated` escape hatch that disables discovery entirely.
- **Format-agnostic, without bloat**: TOML is built in by default; any `serde::Deserialize`
  format is pluggable. We deliberately do **not** bundle every format crate behind a thicket of
  cargo features — that "feature soup" is a named criticism of an existing competitor.

## Target API

The intended shape (to be refined during implementation):

```rust
use clap::Parser;
use clap_layers::Layered;

#[derive(Parser, Layered, Debug)]
#[command(version, about)]
#[layered(file = "myapp.toml", env_prefix = "MYAPP", discover = "xdg")]
struct Config {
    /// Port to listen on
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// Verbosity
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// Config/env only — never a CLI flag
    #[layered(no_cli)]
    retry_budget: u32,

    /// CLI replaces; config appends
    #[arg(long)]
    #[layered(merge = "append")]
    include_paths: Vec<std::path::PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cfg = Config::layered()?;             // flag > env > file > default
    // Config::layered_from(sources) for custom source stacks / testing
    Ok(())
}
```

## How the derive is meant to work

A candidate expansion strategy for the `Layered` derive:

1. Parse the CLI with clap as normal (the user's `#[derive(Parser)]` is untouched).
2. Generate a hidden all-`Option` "partial" mirror of the struct for the serde and env layers.
3. Walk the resulting `ArgMatches` and drop entries whose `value_source()` is
   `ValueSource::DefaultValue`, so that clap defaults don't outrank lower layers incorrectly —
   only *explicitly provided* CLI values survive into the CLI partial.
4. Merge the partials in precedence order (CLI > env > file), applying each field's merge
   strategy, then fall back to the struct's real defaults for anything still unset.
5. Produce the final concrete struct, or a **source-attributed** error if any layer's value
   fails to parse/validate.

Keep the runtime crate near-zero-dependency; the proc-macro crate should lean only on `syn` /
`quote`. See [CLAUDE.md](../CLAUDE.md) for the intended workspace split.

## Non-goals (for now)

- **Subcommands are out of scope for v0.1.** The CLI wants an enum (one subcommand per run) while
  a config file wants a struct (settings for all subcommands); reconciling the two is the hardest
  design problem and is deliberately deferred. The attribute grammar should be designed so this
  can be added later without breaking changes, but it is not built yet.

## Roadmap

- **v0.1** — the derive macro; `flag > env > file > default` precedence with correct
  `ValueSource` semantics; TOML file source; env source with prefix; source-attributed errors;
  `no_cli` / `no_file` markers; the precedence-matrix test suite.
- **v0.2** — per-field merge strategies; `--dump-config`; config-file discovery + `--isolated`;
  a pluggable-format example (e.g. JSON).
- **v0.3** — subcommand support (the enum ↔ struct mapping).

## Prior art & positioning

Several crates occupy adjacent space. This is meant as a fair map, not a takedown — the point is
to be clear about where `clap-layers` aims to differ.

| Crate              | clap-aware | Explicit-vs-default handled | Source-attributed errors |
| ------------------ | ---------- | --------------------------- | ------------------------ |
| **clap-layers**    | yes        | yes (core goal)             | yes (core goal)          |
| twelf              | yes        | partial                     | no                       |
| confique           | no         | n/a                         | yes                      |
| figment            | no (engine) | n/a                        | yes                      |
| clap-config-file   | yes        | partial                     | partial                  |

`figment` is a general layering engine but isn't clap-aware. `confique` is well-maintained but
has no clap integration. The clap-integrated options each miss at least one of the five
requirements above — most commonly explicit-vs-default detection and error attribution. That gap
is the whole reason to build this.

## References

- clap discussion, "Designing for layered configs":
  <https://github.com/clap-rs/clap/discussions/2763>
- clap issue #3113 (same topic): <https://github.com/clap-rs/clap/issues/3113>
- clap's manual-figment docs example (PR #6162):
  <https://github.com/clap-rs/clap/pull/6162>
- Related clap issues: #1695 (help defaults), #2683 (typed `ArgMatches`), #748, #1206
