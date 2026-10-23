//! `sync-kit`: copies masters of the Amalfi kit (silica-design) into
//! `crates/appkit-test/kit`, with the masters they nest. Tests never read silica-design.
//!
//! Prefixes are recorded in `kit/_prefixes` (one per line); without prefixes, every recorded
//! prefix is synced again (after a kit update). `kit/SOURCE.md` records the commit.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{Result, flag, root, sibling};

/// `--ui-kit DIR`: lays the layer trees of ui-kit's `kit/` (newer than silica-design's export)
/// over the fixtures that exist; the PNGs stay as exported. Returns how many trees changed.
fn overlay_ui_kit(dir: &Path, kit: &Path) -> Result<usize> {
    let mut changed = 0;
    let stale_path = kit.join("_stale");
    let mut stale: std::collections::BTreeSet<String> = std::fs::read_to_string(&stale_path)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();
    let mut cache: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    let mut files = Vec::new();
    collect_json(kit, &mut files);
    for file in files {
        let rel = file.strip_prefix(kit).map_err(|e| e.to_string())?;
        let name = rel.with_extension("").to_string_lossy().to_string();
        let Some(category) = name.split('/').next() else {
            continue;
        };
        if !cache.contains_key(category) {
            let path = dir.join("kit").join(format!("{category}.json"));
            let v = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            cache.insert(category.to_string(), v);
        }
        let Some(new) = cache.get(category).map(|c| &c[name.as_str()]) else {
            continue;
        };
        if new.is_null() {
            continue;
        }
        let old: Value = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        if !same(&old, new) {
            let text = serde_json::to_string(new).map_err(|e| e.to_string())?;
            std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
            stale.insert(name.clone());
            changed += 1;
        }
    }
    // Masters the new trees nest that the fixtures lack (no PNG: they are only expanded).
    loop {
        let mut files = Vec::new();
        collect_json(kit, &mut files);
        let mut wanted = Vec::new();
        for file in files {
            let Some(v) = std::fs::read_to_string(&file)
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            else {
                continue;
            };
            let mut nested = Vec::new();
            collect_masters(&v, &mut nested);
            wanted.extend(nested);
        }
        let mut added = false;
        for name in wanted {
            let path = kit.join(format!("{name}.json"));
            if path.exists() {
                continue;
            }
            let Some(category) = name.split('/').next() else {
                continue;
            };
            let new = cache.get(category).map(|c| &c[name.as_str()]);
            let Some(new) = new.filter(|n| !n.is_null()) else {
                continue;
            };
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            let text = serde_json::to_string(new).map_err(|e| e.to_string())?;
            std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
            stale.insert(name.clone());
            changed += 1;
            added = true;
        }
        if !added {
            break;
        }
    }
    std::fs::write(
        &stale_path,
        stale.into_iter().collect::<Vec<_>>().join("\n") + "\n",
    )
    .map_err(|e| e.to_string())?;
    Ok(changed)
}

/// Whether `new` says what `old` says (it may carry more keys: ui-kit adds layout hints).
fn same(old: &Value, new: &Value) -> bool {
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            a.iter().all(|(k, v)| b.get(k).is_some_and(|w| same(v, w)))
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y))
        }
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (a, b) => a == b,
    }
}

fn collect_json(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_json(&p, out);
        } else if p.extension().is_some_and(|x| x == "json")
            && !e.file_name().to_string_lossy().starts_with('_')
        {
            out.push(p);
        }
    }
}

pub fn sync(args: &[String]) -> Result<()> {
    if let Some(i) = args.iter().position(|a| a == "--ui-kit") {
        let dir = args.get(i + 1).ok_or("--ui-kit needs a directory")?;
        let kit = root().join("crates/appkit-test/kit");
        let n = overlay_ui_kit(Path::new(dir), &kit)?;
        println!("{n} layer trees updated from ui-kit");
        return Ok(());
    }
    let design = PathBuf::from(flag(
        args,
        "--design",
        &sibling("silica-design").to_string_lossy(),
    ));
    let kit = root().join("crates/appkit-test/kit");
    std::fs::create_dir_all(&kit).map_err(|e| e.to_string())?;
    let prefixes_file = kit.join("_prefixes");
    let mut prefixes: BTreeSet<String> = std::fs::read_to_string(&prefixes_file)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    let mut skip = false;
    for (i, a) in args.iter().enumerate() {
        if skip {
            skip = false;
            continue;
        }
        if a == "--design" {
            skip = true;
            continue;
        }
        let _ = i;
        prefixes.insert(a.trim_end_matches('/').to_string());
    }
    let index_path = design.join("kit/index.json");
    let index: Value = serde_json::from_str(
        &std::fs::read_to_string(&index_path)
            .map_err(|e| format!("{}: {e}", index_path.display()))?,
    )
    .map_err(|e| e.to_string())?;
    let symbols = index["symbols"].as_array().ok_or("index: no symbols")?;
    let names: Vec<&str> = symbols.iter().filter_map(|s| s["name"].as_str()).collect();
    let mut wanted: BTreeSet<String> = names
        .iter()
        .filter(|n| {
            prefixes
                .iter()
                .any(|p| **n == p.as_str() || n.starts_with(&format!("{p}/")))
        })
        .map(|n| (*n).to_string())
        .collect();
    // Nested masters.
    let mut queue: Vec<String> = wanted.iter().cloned().collect();
    while let Some(name) = queue.pop() {
        let path = design.join("kit/symbols").join(format!("{name}.json"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let mut nested = Vec::new();
        collect_masters(&v, &mut nested);
        for n in nested {
            if names.contains(&n.as_str()) && wanted.insert(n.clone()) {
                queue.push(n);
            }
        }
    }
    let mut copied = 0;
    for name in &wanted {
        for ext in [".json", "@2x.png"] {
            let src = design.join("kit/symbols").join(format!("{name}{ext}"));
            let dst = kit.join(format!("{name}{ext}"));
            if let Some(dir) = dst.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            if src.exists() {
                copy_if_changed(&src, &dst)?;
                copied += 1;
            }
        }
    }
    std::fs::write(
        &prefixes_file,
        prefixes.iter().cloned().collect::<Vec<_>>().join("\n") + "\n",
    )
    .map_err(|e| e.to_string())?;
    let commit = std::process::Command::new("git")
        .arg("-C")
        .arg(&design)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    std::fs::write(
        kit.join("SOURCE.md"),
        format!(
            "# Kit fixtures\n\nCopies of symbol masters of the Silica 1.0 \"Amalfi\" UI kit, from\n\
             [silica-design](https://github.com/silica-os/silica-design) (CC BY-SA 4.0, the Silica\n\
             project), made by `cargo xtask sync-kit`. Do not edit: run the tool again.\n\n\
             silica-design commit: `{}`\n\nThe selected name prefixes are in `_prefixes`; the masters \
             nested in them as symbol instances are copied too.\n",
            commit.trim()
        ),
    )
    .map_err(|e| e.to_string())?;
    println!("{} masters, {copied} files", wanted.len());
    Ok(())
}

fn collect_masters(v: &Value, out: &mut Vec<String>) {
    if let Some(m) = v["master"].as_str() {
        out.push(m.to_string());
    }
    if let Some(children) = v["children"].as_array() {
        for c in children {
            collect_masters(c, out);
        }
    }
}

fn copy_if_changed(src: &Path, dst: &Path) -> Result<()> {
    let a = std::fs::read(src).map_err(|e| format!("{}: {e}", src.display()))?;
    if std::fs::read(dst).ok().as_deref() == Some(a.as_slice()) {
        return Ok(());
    }
    std::fs::write(dst, a).map_err(|e| format!("{}: {e}", dst.display()))
}
