use std::path::{Path, PathBuf};

/// Find a `jamixir/` checkout with a node.sh next to the current directory or the jam-ui binary
pub fn find_jamixir_root() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok();
    let exe = std::env::current_exe().ok();
    [cwd, exe]
        .into_iter()
        .flatten()
        .flat_map(|start| start.ancestors().map(Path::to_path_buf).collect::<Vec<_>>())
        .map(|dir| dir.join("jamixir"))
        .find(|root| root.join("node.sh").is_file())
}

/// Remove .build.lock directory if it exists (from incomplete previous build)
pub fn cleanup_build_lock(jamixir_root: &Path) {
    let build_lock = jamixir_root.join(".build.lock");
    if build_lock.exists() {
        let _ = std::fs::remove_dir_all(&build_lock);
    }
}

/// The node logs in a folder: every `*.log` except summary.log, in natural order (node_2 before node_10)
pub fn find_log_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "log"))
        .filter(|p| p.file_name().is_some_and(|name| name != "summary.log"))
        .collect();
    files.sort_by_cached_key(|p| {
        let stem = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let prefix = stem.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
        let number = stem[prefix.len()..].parse::<u64>().ok();
        (prefix, number, stem)
    });
    Ok(files)
}

/// Replace every `{i}` in a node command with the node index
pub fn node_command(cmd: &str, id: usize) -> String {
    cmd.replace("{i}", &id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_command_substitutes_every_index() {
        assert_eq!(
            node_command("docker run --name node{i} img --port 4000{i}", 3),
            "docker run --name node3 img --port 40003"
        );
        assert_eq!(node_command("./run.sh", 1), "./run.sh");
    }

    #[test]
    fn find_log_files_sorts_naturally_and_skips_summary() {
        let dir = std::env::temp_dir().join(format!("jam-ui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["node_10.log", "node_2.log", "b.log", "a.log", "summary.log", "notes.txt"] {
            std::fs::write(dir.join(name), "").unwrap();
        }
        let names: Vec<String> = find_log_files(&dir)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(names, ["a.log", "b.log", "node_2.log", "node_10.log"]);
    }
}
