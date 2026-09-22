//! The `.botpack` manifest schema.
//!
//! Design rules:
//! - Unknown fields are **ignored** on read (forward compatibility within a
//!   major format version) and **not** emitted on write unless set.
//! - The format version is stamped explicitly; readers reject a different
//!   major version rather than guessing.

use std::fmt;

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Format version this crate implements and emits.
pub const FORMAT_VERSION: Version = Version::new(0, 1, 0);

/// Validated package name.
///
/// Rules: 2–63 bytes, ASCII lowercase alphanumeric plus `-`, must start with
/// a letter, must not end with `-`, no consecutive `-`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PackageName(String);

impl PackageName {
    /// Validates and constructs a package name.
    pub fn new(input: &str) -> Result<Self> {
        validate_name(input).map(|()| Self(input.to_owned()))
    }

    /// Borrowed view of the name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for PackageName {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        validate_name(&value).map(|()| Self(value))
    }
}

impl From<PackageName> for String {
    fn from(value: PackageName) -> Self {
        value.0
    }
}

fn validate_name(s: &str) -> Result<()> {
    let reject = |reason| {
        Err(Error::InvalidName {
            input: s.to_owned(),
            reason,
        })
    };
    let len = s.len();
    if !(2..=63).contains(&len) {
        return reject("length must be 2..=63 bytes");
    }
    let mut prev_dash = false;
    for (i, c) in s.char_indices() {
        let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
        if !ok {
            return reject("only a-z, 0-9 and '-' allowed");
        }
        if i == 0 && !c.is_ascii_lowercase() {
            return reject("must start with a letter");
        }
        if c == '-' {
            if i == len - 1 {
                return reject("must not end with '-'");
            }
            if prev_dash {
                return reject("no consecutive '-'");
            }
            prev_dash = true;
        } else {
            prev_dash = false;
        }
    }
    Ok(())
}

/// Static persona description carried by the package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonaSpec {
    /// Human-readable display name.
    pub display_name: String,
    /// One-paragraph description of the persona's role.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Natural-language system-prompt fragment.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub system_prompt: String,
    /// ISO 639-1 language hints, e.g. `["en", "de"]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
}

/// The manifest: single source of truth inside a `.botpack` archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotpackManifest {
    /// Format version. Readers must reject a different major version.
    pub format_version: Version,
    /// Unique package name.
    pub name: PackageName,
    /// Package (semantic) version.
    pub version: Version,
    /// Persona description.
    pub persona: PersonaSpec,
}

impl BotpackManifest {
    /// Constructs a manifest, stamping the current [`FORMAT_VERSION`].
    pub fn new(name: PackageName, version: Version, persona: PersonaSpec) -> Self {
        Self {
            format_version: FORMAT_VERSION.clone(),
            name,
            version,
            persona,
        }
    }

    /// Semantic validation beyond what the types already guarantee.
    pub fn validate(&self) -> Result<()> {
        if self.format_version.major != FORMAT_VERSION.major {
            return Err(Error::InvalidManifest(format!(
                "unsupported format major version {} (expected {})",
                self.format_version.major, FORMAT_VERSION.major
            )));
        }
        if self.persona.display_name.trim().is_empty() {
            return Err(Error::InvalidManifest(
                "display_name must not be blank".into(),
            ));
        }
        Ok(())
    }

    /// Serializes to pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserializes from a JSON string and runs [`Self::validate`].
    pub fn from_json_str(json: &str) -> Result<Self> {
        let manifest: Self = serde_json::from_str(json)?;
        manifest.validate()?;
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn persona() -> PersonaSpec {
        PersonaSpec {
            display_name: "Test Bot".into(),
            description: String::new(),
            system_prompt: "Be brief.".into(),
            languages: vec!["en".into()],
        }
    }

    #[test]
    fn name_accepts_valid() {
        for s in ["ab", "atlas-caretaker", "a1-b2-c3", "x64"] {
            assert!(PackageName::new(s).is_ok(), "{s} should be valid");
        }
    }

    #[test]
    fn name_rejects_invalid() {
        for s in [
            "",
            "a",
            "-ab",
            "ab-",
            "a--b",
            "Ab",
            "a b",
            "äö",
            "toolongtoolongtoolongtoolongtoolongtoolongtoolongtoolongtoolongtoolong",
        ] {
            assert!(PackageName::new(s).is_err(), "{s} should be invalid");
        }
    }

    #[test]
    fn manifest_roundtrip() {
        let m = BotpackManifest::new(
            PackageName::new("test-bot").unwrap(),
            Version::new(0, 1, 0),
            persona(),
        );
        let json = m.to_json_pretty().unwrap();
        let back = BotpackManifest::from_json_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let m = BotpackManifest::new(
            PackageName::new("test-bot").unwrap(),
            Version::new(0, 1, 0),
            persona(),
        );
        let mut json = m.to_json_pretty().unwrap();
        json.insert(json.len() - 1, ' '); // avoid trailing-brace surgery below
        let json = json.replace("\n}", ",\n  \"future_field\": 42\n}");
        let back = BotpackManifest::from_json_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn major_version_mismatch_rejected() {
        let m = BotpackManifest::new(
            PackageName::new("test-bot").unwrap(),
            Version::new(0, 1, 0),
            persona(),
        );
        let mut json = m.to_json_pretty().unwrap();
        json = json.replace(
            "\"format_version\": \"0.1.0\"",
            "\"format_version\": \"1.0.0\"",
        );
        assert!(BotpackManifest::from_json_str(&json).is_err());
    }

    #[test]
    fn blank_display_name_rejected() {
        let p = PersonaSpec {
            display_name: "  ".into(),
            ..persona()
        };
        let m = BotpackManifest::new(
            PackageName::new("test-bot").unwrap(),
            Version::new(0, 1, 0),
            p,
        );
        assert!(m.validate().is_err());
    }
}
