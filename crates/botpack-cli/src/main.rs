//! `botpack` — CLI for the `.botpack` package format.
//!
//! Subcommands:
//! - `pack <dir> -o <file>`: build a `.botpack` from a directory containing
//!   `manifest.json` plus payload files.
//! - `unpack <file> -o <dir>`: verify and extract a `.botpack`.
//! - `verify <file>`: verify integrity only, print the manifest summary.
//! - `show <file>`: print the manifest JSON exactly as stored in the
//!   archive (byte-preserving; output is a valid pack input).

#![deny(unsafe_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use botpack_archive::{Archive, ArchiveBuilder};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "botpack",
    version,
    about = "Pack, unpack and verify .botpack behavior packages"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build a .botpack from a directory (must contain manifest.json).
    Pack {
        /// Source directory.
        dir: PathBuf,
        /// Output .botpack path.
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Verify and extract a .botpack into a directory.
    Unpack {
        /// .botpack file.
        file: PathBuf,
        /// Destination directory (created if missing).
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Verify integrity and print a manifest summary.
    Verify {
        /// .botpack file.
        file: PathBuf,
    },
    /// Print the manifest JSON.
    Show {
        /// .botpack file.
        file: PathBuf,
    },
}

fn collect_payload(dir: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    let mut out = Vec::new();
    collect_recursive(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_recursive(
    root: &Path,
    cur: &Path,
    out: &mut Vec<(PathBuf, Vec<u8>)>,
) -> Result<(), String> {
    for entry in fs::read_dir(cur).map_err(|e| format!("cannot read {}: {e}", cur.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_recursive(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .expect("walked under root")
                .to_path_buf();
            if rel == Path::new("manifest.json") {
                continue;
            }
            let data =
                fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            out.push((rel, data));
        }
    }
    Ok(())
}

fn cmd_pack(dir: &Path, out: &Path) -> Result<(), String> {
    let path = dir.join("manifest.json");
    let raw = fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut builder = ArchiveBuilder::new()
        .manifest_raw(raw)
        .map_err(|e| format!("invalid manifest.json: {e}"))?;
    for (rel, data) in collect_payload(dir)? {
        let name = rel.to_str().ok_or("non-utf8 payload path")?.to_owned();
        builder = builder.add_file(&name, data).map_err(|e| e.to_string())?;
    }
    builder
        .build_to_file(out)
        .map_err(|e| format!("pack failed: {e}"))?;
    println!("packed {} -> {}", dir.display(), out.display());
    Ok(())
}

fn open(file: &Path) -> Result<Archive, String> {
    Archive::from_file(file).map_err(|e| format!("{}: {e}", file.display()))
}

fn cmd_unpack(file: &Path, out: &Path) -> Result<(), String> {
    let archive = open(file)?;
    fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    // Write the manifest verbatim, so unpacked dirs are byte-identical
    // pack() inputs.
    fs::write(out.join("manifest.json"), archive.manifest_raw())
        .map_err(|e| format!("cannot write manifest.json: {e}"))?;
    for path in archive.file_paths() {
        let dest = out.join(path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let data = archive.file(path).expect("path from iterator");
        fs::write(&dest, data).map_err(|e| format!("cannot write {}: {e}", dest.display()))?;
    }
    println!(
        "verified + extracted {} entries -> {}",
        archive.file_paths().count(),
        out.display()
    );
    Ok(())
}

fn cmd_verify(file: &Path) -> Result<(), String> {
    let archive = open(file)?;
    let m = archive.manifest();
    println!(
        "OK {} v{} (format {}) — {} payload entries",
        m.name,
        m.version,
        m.format_version,
        archive.file_paths().count()
    );
    Ok(())
}

fn cmd_show(file: &Path) -> Result<(), String> {
    let archive = open(file)?;
    // Stored bytes verbatim — no re-serialization, no extra newline.
    // `botpack show f > manifest.json` must be a valid pack input.
    let mut out = std::io::stdout();
    out.write_all(archive.manifest_raw())
        .map_err(|e| format!("cannot write to stdout: {e}"))?;
    out.flush()
        .map_err(|e| format!("cannot flush stdout: {e}"))?;
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Pack { dir, out } => cmd_pack(&dir, &out),
        Command::Unpack { file, out } => cmd_unpack(&file, &out),
        Command::Verify { file } => cmd_verify(&file),
        Command::Show { file } => cmd_show(&file),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
