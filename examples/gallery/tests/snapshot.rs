//! `--snapshot DIR` writes one PNG per page and exits 0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::Command;

#[test]
fn snapshot_writes_one_png_per_page() {
    let dir = std::path::PathBuf::from(format!(
        "/dev/shm/appkit-gallery-snap-{}",
        std::process::id()
    ));
    let status = Command::new(env!("CARGO_BIN_EXE_appkit-gallery"))
        .arg("--snapshot")
        .arg(&dir)
        .arg("--dark")
        .status()
        .unwrap();
    assert!(status.success());
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names.len(), 29, "{names:?}");
    assert_eq!(names.first().map(String::as_str), Some("01-alerts.png"));
    assert!(names.iter().any(|n| n == "28-motion.png"));
    for n in &names {
        let png = std::fs::read(dir.join(n)).unwrap();
        assert_eq!(png.get(1..4), Some(&b"PNG"[..]), "{n}");
        assert!(png.len() > 2000, "{n} is empty");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
