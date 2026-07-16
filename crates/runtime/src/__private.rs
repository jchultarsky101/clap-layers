//! Implementation details used by `#[derive(Layered)]`.
//!
//! **Not public API.** Nothing here is covered by this crate's SemVer
//! guarantees; it exists only so generated code has something to call. Keeping
//! the merge engine here rather than in the expansion means the logic is
//! compiled and tested once, not re-emitted per field.

use crate::{Env, LayeredError};
use clap::ArgMatches;
use clap::parser::ValueSource;
use serde::de::DeserializeOwned;

// Re-exported so it is nameable from generated code and does not trip the
// `private_interfaces` lint in `resolve`'s signature below.
pub use crate::source::ConfigFile;

/// Load the config file named by `#[layered(file = "...")]`, once per parse.
///
/// # Errors
///
/// Returns [`LayeredError::Io`] or [`LayeredError::Parse`] if the file exists
/// but cannot be read or parsed. A missing file yields `Ok(None)`.
pub fn load_file(path: Option<&str>) -> Result<Option<ConfigFile>, LayeredError> {
    match path {
        Some(path) => ConfigFile::load(path),
        None => Ok(None),
    }
}

/// Did the user *actually* supply this argument, as opposed to clap filling in
/// a default?
///
/// This is the crate's core correctness primitive. `ValueSource::EnvVariable`
/// counts as explicit so that clap's own `#[arg(env = "...")]` support keeps
/// beating the config-file layer.
///
/// # Panics
///
/// Panics in debug builds if `id` is not a known argument. The derive only
/// emits this call for fields clap actually parses, never for `#[arg(skip)]`.
#[must_use]
pub fn is_explicit(matches: &ArgMatches, id: &str) -> bool {
    matches!(
        matches.value_source(id),
        Some(ValueSource::CommandLine | ValueSource::EnvVariable)
    )
}

/// Resolve one field across every layer.
///
/// Precedence: explicit CLI flag > environment variable > config file > default.
/// `env_var` is `None` when the field opts out with `no_env` (or no
/// `env_prefix` is configured); `file` is `None` when it opts out with
/// `no_file` (or no file is configured).
///
/// # Errors
///
/// Returns [`LayeredError::Invalid`] if the winning layer supplies a value that
/// cannot be decoded into `T`, attributed to that layer.
pub fn resolve<T: DeserializeOwned>(
    field: &str,
    env_var: Option<&str>,
    env: &Env,
    file: Option<&ConfigFile>,
    cli_explicit: bool,
    cli_value: T,
) -> Result<T, LayeredError> {
    // Layer 1: a flag the user explicitly typed always wins, even when the
    // value they typed equals the default.
    if cli_explicit {
        return Ok(cli_value);
    }

    // Layer 2: environment.
    if let Some(var) = env_var {
        if let Some(raw) = env.get(var) {
            return crate::source::from_env(var, field, raw);
        }
    }

    // Layer 3: config file.
    if let Some(file) = file {
        if let Some(result) = file.get(field) {
            return result;
        }
    }

    // Layer 4: whatever clap defaulted to.
    Ok(cli_value)
}
