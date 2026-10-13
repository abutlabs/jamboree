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
}
