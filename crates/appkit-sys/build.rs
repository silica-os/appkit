//! With the `dynamic` feature, link `libappkit.so` (from `APPKIT_LIB_DIR` or the system).

fn main() {
    println!("cargo:rerun-if-env-changed=APPKIT_LIB_DIR");
    if std::env::var_os("CARGO_FEATURE_DYNAMIC").is_some() {
        if let Some(dir) = std::env::var_os("APPKIT_LIB_DIR") {
            println!("cargo:rustc-link-search=native={}", dir.to_string_lossy());
        }
        println!("cargo:rustc-link-lib=dylib=appkit");
    }
}
