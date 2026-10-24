//! `cargo xtask`: AppKit's development tasks.
//!
//! ```text
//! cargo xtask sync-kit [--design DIR] [PREFIX ...]   copy kit masters into the fixtures
//! cargo xtask sync-kit --ui-kit DIR [PREFIX ...]    lay ui-kit's newer layer trees over them
//!                                                    (and add PREFIX's ui-kit-only masters)
//! cargo xtask sync-sources                          copy tokens, symbols and protocols in
//! cargo xtask check-sources                         compare those copies with the checkouts
//! cargo xtask header                                 regenerate include/appkit.h
//! cargo xtask package [--debug]                      libappkit.so.1 with versioned symbols
//! cargo xtask abi-check                              compare the exported ABI with abi/
//! cargo xtask vm <build|run EXAMPLE|shot PATH>       build for and run in the dev VM
//! cargo xtask bench                                  headless frame-time budgets (release)
//! ```

// serde_json's `value["key"]` returns Null for a missing key: it never panics.
#![allow(clippy::indexing_slicing)]

mod abi;
mod kit;
mod vm;

use std::path::{Path, PathBuf};

/// A task's result.
pub type Result<T> = std::result::Result<T, String>;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or_default();
    let result = match args.first().map(String::as_str) {
        Some("sync-kit") => kit::sync(rest),
        Some("sync-sources") => sync_sources(rest),
        Some("check-sources") => check_sources(rest),
        Some("header") => abi::header(),
        Some("abi-check") => abi::check(rest),
        Some("package") => abi::package(rest).map(|_| ()),
        Some("vm") => vm::vm(rest),
        Some("bench") => bench(),
        _ => Err(
            "usage: cargo xtask <sync-kit|sync-sources|check-sources|header|abi-check|vm> …"
                .to_string(),
        ),
    };
    if let Err(e) = result {
        eprintln!("xtask: {e}");
        std::process::exit(1);
    }
}

/// `cargo xtask bench`: the headless frame-time budgets, in release.
fn bench() -> Result<()> {
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .current_dir(root())
            .args(["run", "--release", "-p", "appkit-bench"])
            .status()
            .map_err(|e| format!("cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("budgets exceeded".into())
    }
}

/// The repository's root.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The value of `--name VALUE` in `args`, or `default`.
pub fn flag(args: &[String], name: &str, default: &str) -> String {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// `~/silica/<dir>`.
pub fn sibling(dir: &str) -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("silica").join(dir)
}

/// The files AppKit copies from its sibling repositories: (source, copy). Tokens, the motion
/// runtime it is checked against and the animations come from silica-os/ui-kit, the symbols
/// font from silica-os/symbols, the protocol bindings from silica-os/compositor.
fn sources(args: &[String]) -> Result<Vec<(PathBuf, PathBuf)>> {
    let ui_kit = PathBuf::from(flag(args, "--ui-kit", &sibling("ui-kit").to_string_lossy()));
    let symbols = PathBuf::from(flag(
        args,
        "--symbols",
        &sibling("symbols").to_string_lossy(),
    ));
    let compositor = PathBuf::from(flag(
        args,
        "--compositor",
        &sibling("compositor").to_string_lossy(),
    ));
    let lib = root().join("crates/appkit");
    let mut pairs = vec![
        (
            ui_kit.join("tokens/tokens.json"),
            lib.join("data/tokens.json"),
        ),
        (
            ui_kit.join("rust/silica_motion.rs"),
            lib.join("data/silica_motion.rs"),
        ),
        (
            symbols.join("fonts/codepoints.json"),
            lib.join("data/symbols-codepoints.json"),
        ),
    ];
    for weight in ["Light", "Regular", "Semibold"] {
        let name = format!("fonts/SilicaSymbols-{weight}.otf");
        pairs.push((symbols.join(&name), lib.join(&name)));
    }
    let protocols = compositor.join("crates/protocols");
    let copy = root().join("crates/silica-protocols");
    pairs.push((protocols.join("build.rs"), copy.join("build.rs")));
    for dir in ["src", "src/menu", "xml"] {
        let entries =
            std::fs::read_dir(protocols.join(dir)).map_err(|e| format!("{}: {e}", dir))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let rel = Path::new(dir).join(entry.file_name());
                pairs.push((path, copy.join(rel)));
            }
        }
    }
    Ok(pairs)
}

/// `cargo xtask sync-sources`: refreshes the copies from the sibling checkouts.
fn sync_sources(args: &[String]) -> Result<()> {
    for (src, copy) in sources(args)? {
        if let Some(dir) = copy.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::copy(&src, &copy).map_err(|e| format!("{}: {e}", src.display()))?;
    }
    println!("sources copied");
    Ok(())
}

/// `cargo xtask check-sources`: compares the copies with the sibling checkouts.
fn check_sources(args: &[String]) -> Result<()> {
    let mut stale = Vec::new();
    for (src, copy) in sources(args)? {
        let a = std::fs::read(&src).map_err(|e| format!("{}: {e}", src.display()))?;
        let b = std::fs::read(&copy).map_err(|e| format!("{}: {e}", copy.display()))?;
        if a != b {
            stale.push(format!("{} differs from {}", copy.display(), src.display()));
        }
    }
    if stale.is_empty() {
        println!("sources up to date");
        Ok(())
    } else {
        Err(stale.join("\n"))
    }
}
