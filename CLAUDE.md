# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this repository.

## What this is

`clap-layers` is a Rust library, published on crates.io, that adds **correct layered
configuration** to [clap](https://crates.io/crates/clap): an explicit CLI flag beats an
env var beats a config file beats a built-in default, driven by a single derive macro over
one struct. The niche's value is **correctness and polish**, not novelty — several
competitors exist and each fails at least one correctness trap. Do not regress on the bar
below to save effort.

## Non-negotiable correctness bar (acceptance criteria)

These are the reasons this crate exists. Every change must preserve them, and each should be
covered by a test.

1. **Explicit-vs-default detection.** A config-file value must override a clap *default* but
   lose to an *explicitly passed* flag — even when the flag's value equals the default. This
   requires plumbing `ArgMatches::value_source() != ValueSource::DefaultValue`. This is the
   core correctness claim; treat any change to merge logic as touching it.
2. **`--help` must keep real defaults.** Fields keep native clap `default_value_t`. Do **not**
   wrap fields in `Option<T>` to detect presence — that makes `--help` print everything as
   optional. Presence detection happens via `ValueSource`, not `Option`.
3. **One struct, no user-visible duplication.** A single user struct drives clap parsing,
   serde deserialization, and env reading. Any partial/shadow ("all-`Option`") struct must be
   generated *inside* the macro and stay invisible to the user.
4. **Source-attributed errors.** Errors name their layer, e.g.
   `invalid value 'foo' for 'port' — from config.toml, line 12`. Never collapse errors into a
   generic post-merge failure that hides which source was wrong.
5. **Field-level control.** Support per-field merge strategies (replace / append / conflict)
   and `no_cli` / `no_file` / `no_env` markers. Avoid one-size-fits-all top-level merge policy.

## Architecture intent

Per the project brief, the target layout is a Cargo **workspace** with two crates:

- a runtime crate (traits, merge engine, sources, errors) — keep dependencies **minimal**
  (near-zero-dep at runtime; avoid pulling in every format crate);
- a proc-macro crate — `syn` / `quote` only.

The repo currently starts as a single crate; splitting into the workspace is an expected
early step. Format support: **TOML built in**, other formats pluggable via
`serde::Deserialize`. Do **not** bundle every format behind a maze of cargo features — a
"cargo features mess" is a named criticism of a competitor.

## Scope discipline

- **Subcommands are out of scope for v0.1.** Design the attribute grammar so subcommand
  support can be added later without breaking changes, but do not implement it now.
- Roadmap: v0.1 = precedence + TOML + env + source-attributed errors + `no_cli`/`no_file`;
  v0.2 = per-field merge strategies, `--dump-config`, discovery + `--isolated`, pluggable
  formats; v0.3 = subcommands.

## Conventions

- Edition 2024; MSRV **1.85** — do not use features newer than the pinned MSRV without
  bumping `rust-version` in `Cargo.toml` and the README.
- Before finishing a change, run: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  and `cargo test`. CI enforces all three (see `.github/workflows/ci.yml`).
- Public API needs rustdoc. Keep doc-comments on config-struct fields meaningful — they are
  the single source of truth that flows into clap help, so treat them as user-facing.
- The flagship test is a **precedence matrix**: `(flag | env | file | none)` × field types.
  Extend it rather than adding ad-hoc one-off tests when you touch precedence.

## Housekeeping

- `docs/PROJECT-BRIEF-clap-layered-config.md` is the design brief. It is **gitignored** and not
  published — keep it local; do not commit or ship it.
- Licensed MIT.
