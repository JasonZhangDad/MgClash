//! Redaction for exported diagnostic bundles.
//!
//! PRD section 25.3 requires a diagnostic bundle to be redacted before it
//! leaves the machine. Two independent passes do that here, because either one
//! alone would leak:
//!
//! - **By key name**, so every credential field a Core config can carry is
//!   masked even when its value is unknown to the caller.
//! - **By exact value**, so a secret the caller *does* know is masked even when
//!   it sits under a key this module has never heard of — the safety net for a
//!   future protocol or a Core that renames a field.

use serde_json::{Map, Value};

/// What every masked value is replaced with.
pub const REDACTED: &str = "[REDACTED]";

/// Values shorter than this are never matched by value: masking a two-character
/// string everywhere would corrupt hostnames and paths across the whole bundle
/// while adding nothing, since short credentials are still caught by key name.
const MIN_MATCHABLE_SECRET: usize = 4;

/// Key names that carry a credential in any Core configuration `MgClash` writes,
/// compared with separators removed so `auth_str`, `authStr`, and `AUTHSTR` are
/// one entry.
const SENSITIVE_KEYS: &[&str] = &[
    "authorization",
    "auth",
    "authstr",
    "credential",
    "password",
    "privatekey",
    "psk",
    "secret",
    "shortid",
    "token",
    "uuid",
];

/// Masks credentials in JSON documents and log text before export.
#[derive(Clone, Debug, Default)]
pub struct DiagnosticRedactor {
    secrets: Vec<String>,
}

impl DiagnosticRedactor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Also masks this exact value wherever it appears, whatever key holds it.
    ///
    /// Values shorter than four characters are ignored; see
    /// [`MIN_MATCHABLE_SECRET`].
    #[must_use]
    pub fn with_secret(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        if value.len() >= MIN_MATCHABLE_SECRET {
            self.secrets.push(value);
        }
        self
    }

    /// Returns a copy of `value` with every credential masked.
    #[must_use]
    pub fn redact_json(&self, value: &Value) -> Value {
        match value {
            Value::Object(fields) => Value::Object(
                fields
                    .iter()
                    .map(|(key, field)| {
                        let redacted = if is_sensitive_key(key) {
                            Value::String(REDACTED.to_owned())
                        } else {
                            self.redact_json(field)
                        };
                        (key.clone(), redacted)
                    })
                    .collect::<Map<_, _>>(),
            ),
            Value::Array(items) => {
                Value::Array(items.iter().map(|item| self.redact_json(item)).collect())
            }
            Value::String(text) => Value::String(self.redact_text(text)),
            other => other.clone(),
        }
    }

    /// Returns `text` with every known secret value masked.
    #[must_use]
    pub fn redact_text(&self, text: &str) -> String {
        self.secrets.iter().fold(text.to_owned(), |text, secret| {
            text.replace(secret.as_str(), REDACTED)
        })
    }
}

/// Compares key names with separators removed and case folded, so a Core that
/// spells a field `auth_str`, `authStr`, or `AUTH-STR` is caught either way.
fn is_sensitive_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect();
    SENSITIVE_KEYS.contains(&normalized.as_str())
}
