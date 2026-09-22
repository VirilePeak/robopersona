//! botpack-core — core types for the `.botpack` behavior-package format.
//!
//! The manifest is the single source of truth inside a `.botpack` archive.
//! This crate defines its schema, validation rules, and (de)serialization.
//!
//! # Example
//! ```
//! use botpack_core::{BotpackManifest, PackageName, PersonaSpec};
//! use semver::Version;
//!
//! let manifest = BotpackManifest::new(
//!     PackageName::new("atlas-caretaker").unwrap(),
//!     Version::new(0, 1, 0),
//!     PersonaSpec {
//!         display_name: "Atlas Caretaker".into(),
//!         description: "Warehouse patrol persona.".into(),
//!         system_prompt: "You are a careful warehouse robot.".into(),
//!         languages: vec!["en".into()],
//!     },
//! );
//!
//! let json = manifest.to_json_pretty().unwrap();
//! let back = BotpackManifest::from_json_str(&json).unwrap();
//! assert_eq!(manifest, back);
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod manifest;

pub use error::{Error, Result};
pub use manifest::{BotpackManifest, FORMAT_VERSION, PackageName, PersonaSpec};
