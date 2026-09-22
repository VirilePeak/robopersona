//! botpack-archive — the `.botpack` container format.
//!
//! Layout (zstd-compressed tar):
//! ```text
//! manifest.json     <- botpack-core::BotpackManifest (MUST be first entry)
//! <payload files>   <- arbitrary relative paths, no `..`, no absolute paths
//! checksums.json    <- {"files": {"<path>": "<sha256-hex>"}} (MUST be last)
//! ```
//!
//! Integrity: the reader recomputes SHA-256 over every payload entry and
//! compares against `checksums.json`. A mismatch aborts the read.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path};

use botpack_core::BotpackManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// All errors produced by botpack-archive.
#[derive(Debug, Error)]
pub enum Error {
    /// Underlying core-crate error (manifest validation etc.).
    #[error(transparent)]
    Core(#[from] botpack_core::Error),

    /// JSON (de)serialization failure inside the container.
    #[error("archive json: {0}")]
    Json(#[from] serde_json::Error),

    /// I/O or container-level failure.
    #[error("archive i/o: {0}")]
    Io(#[from] std::io::Error),

    /// Structural violation of the container layout.
    #[error("invalid archive: {0}")]
    Invalid(String),

    /// A payload entry's digest does not match `checksums.json`.
    #[error("checksum mismatch for {path:?}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Entry path inside the archive.
        path: String,
        /// Digest recorded in `checksums.json`.
        expected: String,
        /// Digest recomputed from the actual bytes.
        actual: String,
    },
}

/// Convenient result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Path of the manifest entry inside the archive.
pub const MANIFEST_ENTRY: &str = "manifest.json";
/// Path of the checksum index entry inside the archive.
pub const CHECKSUMS_ENTRY: &str = "checksums.json";

/// SHA-256 digest index stored as the final archive entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checksums {
    /// Map of entry path → lowercase hex SHA-256.
    pub files: BTreeMap<String, String>,
}

/// Rejects absolute paths and parent traversal (structural safety).
fn validate_entry_path(path: &str) -> Result<()> {
    let p = Path::new(path);
    if p.is_absolute() {
        return Err(Error::Invalid(format!("absolute entry path {path:?}")));
    }
    for c in p.components() {
        match c {
            Component::Normal(_) => {}
            _ => return Err(Error::Invalid(format!("unsafe component in {path:?}"))),
        }
    }
    Ok(())
}

/// Payload paths must be structurally safe and must not collide with the
/// reserved structural entries.
fn validate_payload_path(path: &str) -> Result<()> {
    validate_entry_path(path)?;
    if path == MANIFEST_ENTRY || path == CHECKSUMS_ENTRY {
        return Err(Error::Invalid(format!("reserved entry name {path:?}")));
    }
    Ok(())
}

fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

fn append<W: Write>(tar: &mut tar::Builder<W>, name: &str, data: &[u8]) -> std::io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, name, data)
}

/// Builds a `.botpack` archive.
#[derive(Debug)]
pub struct ArchiveBuilder {
    manifest: Option<BotpackManifest>,
    files: Vec<(String, Vec<u8>)>,
}

impl ArchiveBuilder {
    /// Creates an empty builder.
    pub fn new() -> Self {
        Self {
            manifest: None,
            files: Vec::new(),
        }
    }

    /// Sets the manifest (always written as the first entry).
    pub fn manifest(mut self, manifest: BotpackManifest) -> Self {
        self.manifest = Some(manifest);
        self
    }

    /// Adds a payload file. Paths are validated (relative, no traversal).
    pub fn add_file(mut self, path: &str, contents: impl Into<Vec<u8>>) -> Result<Self> {
        validate_payload_path(path)?;
        self.files.push((path.to_owned(), contents.into()));
        Ok(self)
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        let manifest = self
            .manifest
            .as_ref()
            .ok_or_else(|| Error::Invalid("manifest not set".into()))?;
        manifest.validate()?;

        let mut digests = Checksums {
            files: BTreeMap::new(),
        };
        for (path, data) in &self.files {
            digests.files.insert(path.clone(), sha256_hex(data));
        }

        let mut tar_buf = Vec::new();
        {
            let mut tar = tar::Builder::new(&mut tar_buf);
            append(
                &mut tar,
                MANIFEST_ENTRY,
                &serde_json::to_vec_pretty(manifest)?,
            )?;
            for (path, data) in &self.files {
                append(&mut tar, path, data)?;
            }
            append(
                &mut tar,
                CHECKSUMS_ENTRY,
                &serde_json::to_vec_pretty(&digests)?,
            )?;
            tar.finish()?;
        }
        Ok(tar_buf)
    }

    /// Serializes and zstd-compresses the archive into a byte vector.
    pub fn build(self) -> Result<Vec<u8>> {
        let tar_buf = self.serialize()?;
        let mut out = Vec::new();
        zstd::stream::copy_encode(tar_buf.as_slice(), &mut out, 19)?;
        Ok(out)
    }

    /// Builds and writes the compressed archive to `path`.
    pub fn build_to_file(self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.build()?;
        File::create(path)?.write_all(&bytes)?;
        Ok(())
    }
}

impl Default for ArchiveBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// An opened, fully verified `.botpack` archive.
#[derive(Debug, Clone)]
pub struct Archive {
    manifest: BotpackManifest,
    files: BTreeMap<String, Vec<u8>>,
}

impl Archive {
    /// Reads and verifies a compressed archive from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut decoder = zstd::stream::Decoder::new(bytes)?;
        let mut raw = Vec::new();
        decoder.read_to_end(&mut raw)?;

        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        let mut tar = tar::Archive::new(raw.as_slice());
        for entry in tar.entries()? {
            let mut entry = entry?;
            let name = entry
                .path()?
                .to_str()
                .ok_or_else(|| Error::Invalid("non-utf8 entry path".into()))?
                .to_owned();
            validate_entry_path(&name)?;
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            entries.push((name, data));
        }
        Self::assemble(entries)
    }

    /// Reads and verifies a compressed archive from disk.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let mut f = File::open(path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        Self::from_bytes(&buf)
    }

    fn assemble(entries: Vec<(String, Vec<u8>)>) -> Result<Self> {
        if entries.is_empty() {
            return Err(Error::Invalid("empty archive".into()));
        }
        if entries[0].0 != MANIFEST_ENTRY {
            return Err(Error::Invalid(format!(
                "first entry must be {MANIFEST_ENTRY}, got {:?}",
                entries[0].0
            )));
        }
        let last = entries.last().expect("non-empty checked above");
        if last.0 != CHECKSUMS_ENTRY {
            return Err(Error::Invalid(format!(
                "last entry must be {CHECKSUMS_ENTRY}, got {:?}",
                last.0
            )));
        }
        if entries.len() < 3 {
            return Err(Error::Invalid("archive has no payload entries".into()));
        }

        let manifest: BotpackManifest = serde_json::from_slice(&entries[0].1)?;
        manifest.validate()?;

        let checksums: Checksums = serde_json::from_slice(&last.1)?;

        let mut files = BTreeMap::new();
        for (name, data) in &entries[1..entries.len() - 1] {
            if name == MANIFEST_ENTRY || name == CHECKSUMS_ENTRY {
                return Err(Error::Invalid(format!(
                    "duplicate structural entry {name:?}"
                )));
            }
            let actual = sha256_hex(data);
            let expected = checksums
                .files
                .get(name)
                .ok_or_else(|| Error::Invalid(format!("entry {name:?} missing from checksums")))?;
            if *expected != actual {
                return Err(Error::ChecksumMismatch {
                    path: name.clone(),
                    expected: expected.clone(),
                    actual,
                });
            }
            files.insert(name.clone(), data.clone());
        }

        // Every digest must be consumed exactly once.
        if checksums.files.len() != files.len() {
            return Err(Error::Invalid(format!(
                "checksums list {} entries but archive carries {}",
                checksums.files.len(),
                files.len()
            )));
        }

        Ok(Self { manifest, files })
    }

    /// The verified manifest.
    pub fn manifest(&self) -> &BotpackManifest {
        &self.manifest
    }

    /// Borrowed payload file by path.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    /// All payload paths, sorted.
    pub fn file_paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use botpack_core::{PackageName, PersonaSpec};
    use semver::Version;

    fn manifest() -> BotpackManifest {
        BotpackManifest::new(
            PackageName::new("atlas-caretaker").unwrap(),
            Version::new(0, 1, 0),
            PersonaSpec {
                display_name: "Atlas Caretaker".into(),
                description: "Warehouse patrol persona.".into(),
                system_prompt: "Be careful.".into(),
                languages: vec!["en".into()],
            },
        )
    }

    fn build() -> Vec<u8> {
        ArchiveBuilder::new()
            .manifest(manifest())
            .add_file("prompts/greeting.txt", "hello\n")
            .unwrap()
            .add_file("affect/config.json", b"{\"decay\":0.98}".to_vec())
            .unwrap()
            .build()
            .unwrap()
    }

    #[test]
    fn roundtrip_ok() {
        let bytes = build();
        let a = Archive::from_bytes(&bytes).unwrap();
        assert_eq!(a.manifest().name.as_str(), "atlas-caretaker");
        assert_eq!(a.file("prompts/greeting.txt"), Some(&b"hello\n"[..]));
        assert_eq!(a.file_paths().count(), 2);
    }

    #[test]
    fn corrupted_payload_detected() {
        let mut bytes = build();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xFF;
        // Corruption may also break the zstd stream itself; both are errors.
        assert!(Archive::from_bytes(&bytes).is_err());
    }

    #[test]
    fn traversal_and_reserved_names_rejected() {
        assert!(
            ArchiveBuilder::new()
                .add_file("../escape.txt", "x")
                .is_err()
        );
        assert!(ArchiveBuilder::new().add_file("/abs.txt", "x").is_err());
        assert!(ArchiveBuilder::new().add_file(MANIFEST_ENTRY, "x").is_err());
        assert!(
            ArchiveBuilder::new()
                .add_file(CHECKSUMS_ENTRY, "x")
                .is_err()
        );
    }

    #[test]
    fn missing_manifest_rejected() {
        let mut tar_buf = Vec::new();
        {
            let mut tar = tar::Builder::new(&mut tar_buf);
            append(&mut tar, "a.txt", b"x").unwrap();
            append(
                &mut tar,
                CHECKSUMS_ENTRY,
                &serde_json::to_vec(&Checksums {
                    files: BTreeMap::new(),
                })
                .unwrap(),
            )
            .unwrap();
            tar.finish().unwrap();
        }
        let mut comp = Vec::new();
        zstd::stream::copy_encode(tar_buf.as_slice(), &mut comp, 3).unwrap();
        assert!(Archive::from_bytes(&comp).is_err());
    }

    #[test]
    fn file_roundtrip_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.botpack");
        ArchiveBuilder::new()
            .manifest(manifest())
            .add_file("x.txt", "y")
            .unwrap()
            .build_to_file(&path)
            .unwrap();
        let a = Archive::from_file(&path).unwrap();
        assert_eq!(a.file("x.txt"), Some(&b"y"[..]));
    }
}
