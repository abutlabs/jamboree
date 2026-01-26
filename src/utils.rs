use std::path::PathBuf;

/// Get the jamixir root directory path
pub fn get_jamixir_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let parent_dir = std::path::Path::new(manifest_dir).parent().unwrap();
    parent_dir.join("jamixir")
}

/// Remove .build.lock directory if it exists (from incomplete previous build)
pub fn cleanup_build_lock() {
    let jamixir_root = get_jamixir_root();
    let build_lock = jamixir_root.join(".build.lock");
    if build_lock.exists() {
        let _ = std::fs::remove_dir_all(&build_lock);
    }
}
