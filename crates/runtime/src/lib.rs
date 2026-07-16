//! Correct layered configuration for [clap](https://crates.io/crates/clap).
//!
//! One derive macro gives a clap application layered configuration with the
//! precedence users actually expect:
//!
//! | Priority | Layer             | Example                          |
//! | -------- | ----------------- | -------------------------------- |
//! | 1        | Explicit CLI flag | `--port 8080`                    |
//! | 2        | Environment var   | `MYAPP_PORT=8080`                |
//! | 3        | Config file       | `port = 8080` in `myapp.toml`    |
//! | 4        | Built-in default  | `#[arg(default_value_t = 3000)]` |
//!
//! The subtle part — and the reason this crate exists — is layer 1 versus
//! layer 4. A config-file value must override a clap *default*, but must lose
//! to a flag the user *explicitly typed*, **even when the typed value happens to
//! equal the default**. `clap-layers` gets this right by reading clap's
//! [`ValueSource`](clap::parser::ValueSource) rather than by wrapping every
//! field in `Option<T>`, so `--help` keeps showing real defaults.
//!
//! # Example
//!
//! ```
//! use clap::Parser;
//! use clap_layers::{Env, Layered};
//!
//! #[derive(Parser, Layered, Debug)]
//! #[layered(file = "myapp.toml", env_prefix = "MYAPP")]
//! struct Config {
//!     /// Port to listen on
//!     #[arg(long, default_value_t = 3000)]
//!     port: u16,
//! }
//!
//! // `Config::layered()` reads the real process arguments and environment.
//! // `layered_from` is the same engine with both injected, which is what makes
//! // configuration testable:
//! let env = Env::from_iter([("MYAPP_PORT", "8080")]);
//!
//! // No flag typed: the environment wins over the default.
//! let cfg = Config::layered_from(["myapp"], &env)?;
//! assert_eq!(cfg.port, 8080);
//!
//! // Flag explicitly typed: it beats the environment.
//! let cfg = Config::layered_from(["myapp", "--port", "9999"], &env)?;
//! assert_eq!(cfg.port, 9999);
//!
//! // Even when the typed value *equals* the default, it is still explicit.
//! let cfg = Config::layered_from(["myapp", "--port", "3000"], &env)?;
//! assert_eq!(cfg.port, 3000);
//! # Ok::<(), clap_layers::LayeredError>(())
//! ```
//!
//! # Reporting errors
//!
//! [`LayeredError`] carries its message in its [`Display`](std::fmt::Display)
//! form. Rust prints the `Debug` form for a `Result` returned from `main`, so
//! `let cfg = Config::layered()?` in `main` reports
//! `Invalid { field: "port", .. }` rather than the attributed message. Print it
//! yourself instead:
//!
//! ```no_run
//! # use clap::Parser;
//! # use clap_layers::Layered;
//! # #[derive(Parser, Layered, Debug)]
//! # struct Config {
//! #     #[arg(long, default_value_t = 3000)]
//! #     port: u16,
//! # }
//! let cfg = Config::layered().unwrap_or_else(|e| {
//!     eprintln!("configuration error: {e}");
//!     std::process::exit(1);
//! });
//! ```
//!
//! ```text
//! configuration error: invalid value 'banana' for 'port' — from environment variable MYAPP_PORT (invalid type: string, expected u16)
//! ```
//!
//! # Field types
//!
//! Values from the environment and config file are decoded with
//! [`serde`], so any field type implementing [`serde::Deserialize`] works —
//! including `u16`, `bool`, `String`, `Vec<T>`, `Option<T>`, and `#[derive(Deserialize)]`
//! enums. No `FromStr` bound is required.
//!
//! # Per-field control
//!
//! - `#[layered(no_env)]` — never read this field from the environment.
//! - `#[layered(no_file)]` — never read this field from the config file.
//! - `#[layered(no_cli)]` — never expose this field as a CLI flag. Requires
//!   `#[arg(skip)]` so clap leaves the field alone; the macro enforces this.
//!
//! # Environment variables
//!
//! The environment layer is **only active when `env_prefix` is set**. Without a
//! prefix, a field named `path` would read the ambient `PATH`, so `clap-layers`
//! disables the layer rather than guess. Names are `PREFIX_FIELD` uppercased:
//! `env_prefix = "MYAPP"` maps field `db_password` to `MYAPP_DB_PASSWORD`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
// A library returns `Result`; it must not abort its caller's process. Relaxed
// under `cfg(test)`, where unwrapping is how a test asserts.
#![cfg_attr(
    not(test),
    warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

mod source;

#[doc(hidden)]
pub mod __private;

/// Re-export of the `clap` version this crate is built against.
///
/// [`LayeredError::Cli`] wraps a [`clap::Error`], so this re-export lets you
/// handle it without depending on a matching `clap` version yourself.
pub use clap;

pub use clap_layers_proc::Layered;

/// Which layer supplied a configuration value.
///
/// Used in [`LayeredError::Invalid`] to attribute a bad value to the exact
/// layer that produced it.
///
/// Only the layers this crate decodes values for appear here. A malformed CLI
/// argument is clap's to report, and surfaces as [`LayeredError::Cli`], so
/// there is no `CliFlag` variant to construct. This enum is `#[non_exhaustive]`,
/// so `--dump-config` can add the remaining layers in a later version without a
/// breaking change.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SourceLayer {
    /// An environment variable, holding the variable's name.
    EnvVar(String),
    /// A configuration file, holding its path and the 1-based line number.
    ConfigFile {
        /// Path to the configuration file.
        path: PathBuf,
        /// 1-based line number the value appears on.
        line: usize,
    },
}

impl std::fmt::Display for SourceLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceLayer::EnvVar(var) => write!(f, "environment variable {var}"),
            SourceLayer::ConfigFile { path, line } => {
                write!(f, "{}, line {line}", path.display())
            }
        }
    }
}

/// Errors produced while loading layered configuration.
///
/// Every variant names the layer it came from, so a bad value is never reported
/// as an anonymous post-merge failure.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum LayeredError {
    /// Command-line parsing failed, or clap was asked to display help/version.
    ///
    /// [`Layered::layered`] never returns this — it defers to
    /// [`clap::Error::exit`], matching [`clap::Parser::parse`]. It is only
    /// returned by [`Layered::layered_from`].
    #[error(transparent)]
    Cli(#[from] clap::Error),

    /// The configuration file exists but could not be read.
    ///
    /// A *missing* config file is not an error; the layer is simply skipped.
    #[error("could not read config file '{}': {source}", path.display())]
    Io {
        /// Path to the file that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The configuration file is not valid TOML.
    #[error("could not parse config file '{}' at line {line}, column {column}: {message}", path.display())]
    Parse {
        /// Path to the offending file.
        path: PathBuf,
        /// 1-based line number of the syntax error.
        line: usize,
        /// 1-based column number of the syntax error.
        column: usize,
        /// Message from the TOML parser.
        message: String,
    },

    /// A layer supplied a value that could not be decoded into the field's type.
    #[error("invalid value '{value}' for '{field}' — from {layer} ({message})")]
    Invalid {
        /// Name of the field that could not be populated.
        field: String,
        /// The offending value, as written in the source.
        value: String,
        /// The layer that supplied the value.
        layer: SourceLayer,
        /// Why decoding failed, e.g. `invalid type: string, expected u16`.
        message: String,
    },
}

/// A snapshot of environment variables for the environment layer.
///
/// Taking the environment as an explicit value — rather than reading the
/// process environment deep inside the merge engine — is what makes
/// [`Layered::layered_from`] testable and safe to run in parallel tests.
///
/// # Examples
///
/// ```
/// use clap_layers::Env;
///
/// let env = Env::from_iter([("MYAPP_PORT", "8080")]);
/// assert_eq!(env.get("MYAPP_PORT"), Some("8080"));
/// assert_eq!(env.get("MYAPP_HOST"), None);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Env {
    vars: BTreeMap<String, String>,
}

impl Env {
    /// Capture the current process environment.
    ///
    /// Variables whose name or value is not valid UTF-8 are skipped.
    #[must_use]
    pub fn from_system() -> Self {
        Self {
            vars: std::env::vars_os()
                .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
                .collect(),
        }
    }

    /// An environment with no variables set.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Look up a variable by name.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(String::as_str)
    }
}

/// Generic over the key and value types so both `("A", "1")` and owned
/// `String` pairs work, which keeps test setup free of `to_string()` noise.
impl<K, V> FromIterator<(K, V)> for Env
where
    K: Into<String>,
    V: Into<String>,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self {
            vars: iter
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }
}

/// Load a struct from all configuration layers.
///
/// Implemented by `#[derive(Layered)]`; do not implement it by hand. The
/// derived type must also derive [`clap::Parser`].
pub trait Layered: Sized {
    /// Load configuration from the process arguments and environment.
    ///
    /// This is the entry point for applications. Like [`clap::Parser::parse`],
    /// it does **not** return CLI errors: on a bad flag it prints a diagnostic
    /// and exits non-zero, and on `--help` / `--version` it prints and exits
    /// zero. Errors from the file and environment layers are still returned.
    ///
    /// Use [`Layered::layered_from`] in tests.
    ///
    /// # Errors
    ///
    /// Returns [`LayeredError`] if the config file is unreadable or malformed,
    /// or if any layer supplies a value that cannot be decoded.
    fn layered() -> Result<Self, LayeredError> {
        match Self::layered_from(std::env::args_os(), &Env::from_system()) {
            Err(LayeredError::Cli(e)) => e.exit(),
            other => other,
        }
    }

    /// Load configuration from explicit arguments and an explicit environment.
    ///
    /// The testable form of [`Layered::layered`]: nothing is read from process
    /// globals, so tests are hermetic and can run in parallel. CLI errors are
    /// returned as [`LayeredError::Cli`] rather than exiting.
    ///
    /// # Errors
    ///
    /// Returns [`LayeredError::Cli`] if argument parsing fails (including
    /// `--help`), or another [`LayeredError`] if a layer is unreadable,
    /// malformed, or supplies an undecodable value.
    fn layered_from<I, T>(args: I, env: &Env) -> Result<Self, LayeredError>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_layer_display() {
        assert_eq!(
            SourceLayer::EnvVar("MYAPP_PORT".to_string()).to_string(),
            "environment variable MYAPP_PORT"
        );
        assert_eq!(
            SourceLayer::ConfigFile {
                path: PathBuf::from("config.toml"),
                line: 12,
            }
            .to_string(),
            "config.toml, line 12"
        );
    }

    /// The exact wording promised by the project's correctness bar.
    #[test]
    fn invalid_error_is_source_attributed() {
        let err = LayeredError::Invalid {
            field: "port".to_string(),
            value: "foo".to_string(),
            layer: SourceLayer::ConfigFile {
                path: PathBuf::from("config.toml"),
                line: 12,
            },
            message: "invalid type: string, expected u16".to_string(),
        };
        assert!(
            err.to_string()
                .starts_with("invalid value 'foo' for 'port' — from config.toml, line 12"),
            "got: {err}"
        );
    }

    #[test]
    fn invalid_error_names_the_env_var() {
        let err = LayeredError::Invalid {
            field: "port".to_string(),
            value: "banana".to_string(),
            layer: SourceLayer::EnvVar("MYAPP_PORT".to_string()),
            message: "invalid digit".to_string(),
        };
        assert!(
            err.to_string()
                .contains("from environment variable MYAPP_PORT"),
            "got: {err}"
        );
    }

    #[test]
    fn io_error_names_the_path() {
        let err = LayeredError::Io {
            path: PathBuf::from("config.toml"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        };
        assert!(err.to_string().contains("config.toml"), "got: {err}");
    }

    #[test]
    fn env_from_iter_and_get() {
        let env = Env::from_iter([("A".to_string(), "1".to_string())]);
        assert_eq!(env.get("A"), Some("1"));
        assert_eq!(env.get("B"), None);
        assert_eq!(Env::empty().get("A"), None);
    }

    #[test]
    fn env_from_system_reads_process_env() {
        // `Env::from_system` is the one place that touches process globals.
        let env = Env::from_system();
        // PATH is set on every platform CI runs on.
        assert!(env.get("PATH").is_some() || env.get("Path").is_some());
    }
}
