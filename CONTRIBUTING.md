# Contributing to clap-layers

Thanks for your interest in contributing! This project aims to win on **correctness and
polish**, so contributions that add tests and sharpen error messages are especially welcome.

## Getting started

```sh
git clone https://github.com/jchultarsky101/clap-layers
cd clap-layers
cargo test
```

## Before you open a pull request

Please make sure the following all pass locally — CI runs the same checks:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

The minimum supported Rust version (MSRV) is **1.85**. Don't use language or standard-library
features newer than that without bumping `rust-version` in `Cargo.toml` and updating the README.

## Adding a dependency

Every dependency is compile time, audit surface and SemVer risk, so each one needs a reason.
CI checks the supply chain on every push and weekly on a timer; run the same checks locally:

```sh
cargo deny check   # advisories, licenses, bans, sources — see deny.toml
cargo audit        # RustSec advisories
```

`deny.toml` allows only `MIT`, `Apache-2.0` and `Unicode-3.0`. Widening that list is a
licensing decision for everyone downstream of this crate, not a formality — if a dependency
needs it, say why in the pull request.

Note that `cargo deny check advisories` can start failing without anyone touching the code,
because an advisory was published against a crate already in `Cargo.lock`. That is the check
working, not a flaky test.

## Correctness bar

This crate exists to get layered-configuration precedence *right*. Please read
[CLAUDE.md](CLAUDE.md) for the five non-negotiable correctness criteria (explicit-vs-default
detection, `--help` preservation, single-struct source of truth, source-attributed errors, and
field-level control). Changes that touch merge/precedence logic should extend the precedence
matrix test suite.

## Commit and PR guidelines

- Keep PRs focused; one logical change per PR.
- Add or update tests for behavior changes.
- Update `CHANGELOG.md` under the `## [Unreleased]` heading.
- Describe *why*, not just *what*, in the PR description.

## Reporting bugs

Open an issue with a minimal reproduction — ideally a small config struct plus the CLI args,
env vars, and config file contents that produce the wrong result.

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE).
