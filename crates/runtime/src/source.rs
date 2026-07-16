//! The config-file and environment layers, and decoding values out of them.

use crate::{LayeredError, SourceLayer};
use serde::de::{DeserializeOwned, IntoDeserializer};
use std::collections::BTreeMap;
use std::path::PathBuf;
use toml::{Spanned, Value};

/// A parsed TOML configuration file, retaining each value's source span so a
/// bad value can be attributed to an exact line.
///
/// Loaded once per `layered_from` call and shared by every field.
#[derive(Debug)]
pub struct ConfigFile {
    path: PathBuf,
    /// Kept so error paths can turn a span back into a line and column.
    content: String,
    values: BTreeMap<String, Spanned<Value>>,
}

impl ConfigFile {
    /// Read and parse a config file.
    ///
    /// A missing file yields `Ok(None)`: config files are optional. Any other
    /// I/O failure is a real error — silently ignoring an unreadable file would
    /// hide a misconfiguration.
    ///
    /// # Errors
    ///
    /// Returns [`LayeredError::Io`] if the file exists but cannot be read, or
    /// [`LayeredError::Parse`] if it is not valid TOML.
    pub fn load(path: impl Into<PathBuf>) -> Result<Option<Self>, LayeredError> {
        let path = path.into();
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(LayeredError::Io { path, source }),
        };

        let values: BTreeMap<String, Spanned<Value>> = toml::from_str(&content).map_err(|e| {
            let (line, column) = position_of(&content, e.span().map_or(0, |span| span.start));
            LayeredError::Parse {
                path: path.clone(),
                line,
                column,
                message: e.message().to_string(),
            }
        })?;

        Ok(Some(Self {
            path,
            content,
            values,
        }))
    }

    /// Decode `field` from this file, if the file sets it.
    #[must_use]
    pub fn get<T: DeserializeOwned>(&self, field: &str) -> Option<Result<T, LayeredError>> {
        let spanned = self.values.get(field)?;
        let value = spanned.get_ref();

        Some(
            T::deserialize(value.clone().into_deserializer()).map_err(|e| {
                // Resolve the position only when reporting an error, rather than
                // for every field the file happens to set.
                let (line, _) = position_of(&self.content, spanned.span().start);
                LayeredError::Invalid {
                    field: field.to_string(),
                    value: display_value(value),
                    layer: SourceLayer::ConfigFile {
                        path: self.path.clone(),
                        line,
                    },
                    message: e.message().to_string(),
                }
            }),
        )
    }
}

/// Decode `raw` from an environment variable into `T`.
///
/// The value is first read as a TOML value expression, so `8080`, `true` and
/// `["a", "b"]` decode to the types you would expect. If that fails, it is
/// treated as a bare string, so `MYAPP_NAME=hello` works without quoting.
pub(crate) fn from_env<T: DeserializeOwned>(
    var: &str,
    field: &str,
    raw: &str,
) -> Result<T, LayeredError> {
    if let Ok(value) = T::deserialize(toml::de::ValueDeserializer::new(raw)) {
        return Ok(value);
    }

    T::deserialize(Value::String(raw.to_string()).into_deserializer()).map_err(
        |e: toml::de::Error| LayeredError::Invalid {
            field: field.to_string(),
            value: raw.to_string(),
            layer: SourceLayer::EnvVar(var.to_string()),
            message: e.message().to_string(),
        },
    )
}

/// Render a value the way the user wrote it, so `port = "foo"` reports `foo`
/// rather than `"foo"`.
fn display_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Convert a byte offset into a 1-based `(line, column)` pair.
///
/// The column counts **characters, not bytes**, which is the convention `toml`
/// uses in its own diagnostics: for a line containing multi-byte text the two
/// disagree, and reporting a byte column would contradict every other tool
/// looking at the same file.
///
/// Runs only on error paths, so the linear scan is not worth optimising.
fn position_of(content: &str, offset: usize) -> (usize, usize) {
    // Clamp defensively. A span past the end, or landing inside a multi-byte
    // character, must not panic in a library.
    let mut offset = offset.min(content.len());
    while offset > 0 && !content.is_char_boundary(offset) {
        offset -= 1;
    }

    let prefix = &content[..offset];
    let line_start = prefix.rfind('\n').map_or(0, |i| i + 1);
    let line = prefix.matches('\n').count() + 1;
    let column = content[line_start..offset].chars().count() + 1;
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_maps_offsets_to_lines_and_columns() {
        let content = "a = 1\nbb = 2\n\nccc = 3\n";
        assert_eq!(position_of(content, 0), (1, 1));
        assert_eq!(position_of(content, 4), (1, 5));
        // First byte of line 2.
        assert_eq!(position_of(content, 6), (2, 1));
        // The blank line 3.
        assert_eq!(position_of(content, 13), (3, 1));
        assert_eq!(position_of(content, 14), (4, 1));
    }

    #[test]
    fn position_handles_empty_content() {
        assert_eq!(position_of("", 0), (1, 1));
    }

    #[test]
    fn position_counts_columns_in_characters_not_bytes() {
        // 'é' and 'ø' are two bytes each, so a byte column would drift.
        let content = "x = 1\n\"kéy_ø\" = 2\n";
        let offset = content.find("= 2").unwrap();
        let (line, column) = position_of(content, offset);
        assert_eq!(line, 2);
        // "kéy_ø" quoted is 7 characters, then a space: the `=` is character 9.
        assert_eq!(column, 9, "column must count characters, not bytes");
    }

    /// Pins our column convention to `toml`'s own, so a value we report can be
    /// cross-checked against any other tool reading the same file.
    #[test]
    fn position_agrees_with_tomls_own_diagnostics() {
        let content = "x = 1\n\"kéy_ünïcödé_ø\" = = 2\n";
        let err = toml::from_str::<toml::Table>(content).unwrap_err();
        let (line, column) = position_of(content, err.span().unwrap().start);

        // toml renders "TOML parse error at line 2, column 19".
        let rendered = err.to_string();
        let first = rendered.lines().next().unwrap();
        assert!(
            first.contains(&format!("line {line}, column {column}")),
            "ours: line {line}, column {column}; toml: {first}"
        );
    }

    #[test]
    fn position_does_not_panic_on_out_of_range_or_split_offsets() {
        let content = "a = \"é\"\n";

        // Past the end clamps to the end of input, which sits just after the
        // trailing newline: the first column of the (empty) next line.
        assert_eq!(position_of(content, 9_999), (2, 1));

        // An offset inside the two bytes of 'é' must clamp back to the
        // character boundary rather than panic on a bad slice.
        let inside = content.find('é').unwrap() + 1;
        assert!(!content.is_char_boundary(inside));
        assert_eq!(position_of(content, inside), (1, 6));
    }

    #[test]
    fn env_decodes_toml_value_expressions() {
        assert_eq!(from_env::<u16>("V", "port", "8080").unwrap(), 8080);
        assert!(from_env::<bool>("V", "verbose", "true").unwrap());
        assert_eq!(
            from_env::<Vec<String>>("V", "tags", r#"["a", "b"]"#).unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(
            from_env::<Option<u16>>("V", "port", "8080").unwrap(),
            Some(8080)
        );
    }

    #[test]
    fn env_falls_back_to_bare_strings() {
        // `hello` is not a valid TOML value expression, but is an obvious string.
        assert_eq!(from_env::<String>("V", "name", "hello").unwrap(), "hello");
        assert_eq!(
            from_env::<String>("V", "name", "hello world").unwrap(),
            "hello world"
        );
        // A bare `true` is a valid TOML bool, but the field wants a String, so
        // the fallback must still produce the literal text.
        assert_eq!(from_env::<String>("V", "name", "true").unwrap(), "true");
        assert_eq!(from_env::<String>("V", "name", "8080").unwrap(), "8080");
    }

    #[test]
    fn env_reports_undecodable_values_against_the_variable() {
        let err = from_env::<u16>("MYAPP_PORT", "port", "banana").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid value 'banana' for 'port'"), "{msg}");
        assert!(msg.contains("environment variable MYAPP_PORT"), "{msg}");
    }
}
