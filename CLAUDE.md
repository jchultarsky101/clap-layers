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

## Rust library development practices

These are standing expectations for every change. Treat them as part of "done," not optional
polish. They exist because this crate's whole value proposition is correctness and quality —
a sloppy library loses to the incumbents regardless of features.

### The per-change loop (run in this order, every time)

1. `cargo build` (or `cargo check`) — must compile clean.
2. `cargo fmt --all` — format. CI runs `--check` and fails on any diff, so format locally.
3. `cargo clippy --all-targets --all-features -- -D warnings` — **run after every build**.
   Warnings are errors here; fix them, don't `#[allow]` them away without a written reason.
4. `cargo test --all-features` — unit, integration, **and doctests**. Doctests count.
5. `cargo doc --no-deps --all-features` with `RUSTDOCFLAGS="-D warnings"` — no broken
   intra-doc links, no missing docs.

Do not report work as complete until steps 1–5 pass. If you cannot run them, say so
explicitly rather than implying they passed.

### Testing & coverage (always generate tests; aim for full coverage)

- **Every change ships with tests.** New public behavior → new tests. Bug fix → a regression
  test that fails before the fix and passes after. No behavior change lands untested.
- **Coverage is measured, not assumed.** Use `cargo llvm-cov` (`cargo llvm-cov --all-features
  --workspace`, or `--html` for a report). Target **≥ 90% line coverage** on the runtime crate;
  the precedence/merge engine should be effectively 100%. If coverage drops, add tests before
  moving on. Note honestly what is uncovered and why.
- **The precedence matrix is the flagship test:** `(flag | env | file | none)` × field types
  (`u16`, `bool`, `String`, `Vec<T>`, `Option<T>`, enums). Extend this matrix rather than
  adding scattered one-off tests when you touch precedence.
- **Proc-macro crates need compile-fail tests.** Use [`trybuild`](https://docs.rs/trybuild) to
  assert that misuse (bad `#[layered(...)]` attributes, unsupported field types) produces a
  *good* error message, and use `macrotest`/`cargo expand` to sanity-check generated code.
  A derive macro's error messages are part of its public UX — test them.
- **Property-based tests** (`proptest`) are well-suited to the merge engine: for any generated
  stack of layers, the highest-priority set value must win. Prefer this over hand-enumerating.
- **Integration tests** live in `tests/` and must exercise the crate as an external user would
  (`use clap_layers::...`), not reach into private internals.
- Test against **MSRV (1.85)** as well as stable — CI already does; keep it green.

### API design (follow the Rust API Guidelines)

- Conform to the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) checklist.
  Highlights that bite hardest:
  - Public error types implement `std::error::Error` + `Display` + `Debug`. **Do not expose
    `anyhow`/`Box<dyn Error>` in the library's public API** — that's for the examples/binaries.
    Prefer a concrete error enum (hand-rolled or `thiserror`).
  - Mark growable public enums/structs `#[non_exhaustive]` (especially the error type and any
    config-source enum) so adding variants isn't a breaking change.
  - Derive the common traits where sensible (`Debug`, `Clone`, `PartialEq`, `Eq`) —
    C-COMMON-TRAITS. `Debug` on all public types is effectively mandatory.
  - Keep the public surface **minimal**: `pub(crate)` by default, `pub` only when intended.
    Everything `pub` is a maintenance commitment under SemVer.
  - Re-export any third-party types that appear in your public signatures so users don't need
    to add a matching dependency.
- Add `#![forbid(unsafe_code)]` unless a concrete, documented need for `unsafe` arises.
- **No `panic!`/`unwrap`/`expect` in library code paths** — return `Result`. Panics are for
  genuinely-unreachable invariants only, and should say why.
- Feature flags must be **additive** (enabling one never breaks another); document each in the
  crate docs. Format support (TOML/JSON/…) is the main axis — keep it clean, not a maze.

### Documentation (keep README, rustdoc, and CHANGELOG in sync)

- `#![deny(missing_docs)]` at the crate root — every public item is documented, with at least
  one runnable example on the crate root and on the primary entry points.
- **Document all user-facing changes in `README.md`.** If a change alters usage — a new
  attribute, a renamed method, a precedence tweak — the README example/table must reflect it in
  the same change. A README that lies about the API is worse than none.
- **Maintain `CHANGELOG.md` for every released change** ([Keep a Changelog] format): add an
  entry under `## [Unreleased]` as part of the change, then move it under a versioned heading at
  release time. Internal-only refactors can be omitted; anything a user could observe cannot.
- Configure docs.rs to build all features by adding to `Cargo.toml`:
  ```toml
  [package.metadata.docs.rs]
  all-features = true
  rustdoc-args = ["--cfg", "docsrs"]
  ```
  and gate feature-flag docs with `#[cfg_attr(docsrs, doc(cfg(...)))]`.
- Keep the README and the crate-root docs from drifting — consider `#![doc = include_str!("../README.md")]`
  (with doctests on the README) so there is a single source of truth.

### Versioning & releasing (SemVer discipline)

- Follow [SemVer](https://semver.org/) strictly. Pre-1.0, a breaking change bumps the **minor**
  (0.x.0); after 1.0, it bumps the major. When unsure whether a change is breaking, assume it is.
- Run [`cargo-semver-checks`](https://github.com/obi1kenobi/cargo-semver-checks) before any
  release to catch accidental API breaks; wire it into CI once there's a published baseline.
- Before publishing: `cargo publish --dry-run` and inspect `cargo package --list` to confirm no
  stray files (the design brief in `docs/` must never ship — it's excluded).
- Tag releases `vX.Y.Z` matching `Cargo.toml`. Consider `cargo-release` or `release-plz` to
  automate the version-bump / changelog / tag / publish flow.

### Dependency & supply-chain hygiene

- **Keep dependencies minimal** — this is a stated design goal, and each dep is compile time +
  audit surface + SemVer risk. Justify every addition.
- [`cargo-deny`](https://embarkstudios.github.io/cargo-deny/) runs in CI on every push and
  weekly on a schedule (an advisory can land against an unchanged `Cargo.lock`). It covers
  RustSec advisories, licences, duplicates and sources; `deny.toml` holds the policy and has
  no exceptions — keep it that way. It deliberately replaces `cargo-audit`, which reads the
  same RustSec database and would be a second configuration to keep honest for no extra
  signal. Dependabot is already configured.
- Commit `Cargo.lock` (done) so CI and coverage are reproducible.
- State an MSRV policy and honor it: MSRV is **1.85**; raising it is a minor-version, changelog-
  worthy event, and requires updating `rust-version`, the README, and the CI matrix together.

### Consider adding as the code grows

- A `[lints]` table in `Cargo.toml` (or `#![warn(...)]` at the crate root) to centralize
  `missing_docs`, `unsafe_code`, and selected `clippy::pedantic`/`clippy::cargo` lints.
- CI jobs for coverage upload, `cargo-deny`, and (post-baseline) `cargo-semver-checks`.
- A `SECURITY.md` and a tag-triggered publish workflow once the crate is on crates.io.

[Keep a Changelog]: https://keepachangelog.com/en/1.1.0/

## Housekeeping

- `docs/PROJECT-BRIEF-clap-layered-config.md` is the design brief. It is **gitignored** and not
  published — keep it local; do not commit or ship it.
- Licensed MIT.
