use std::path::{Path, PathBuf};

/// The node logs in a folder: every `*.log`, in natural order (node_2 before node_10)
pub fn find_log_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "log"))
        .collect();
    files.sort_by_cached_key(|p| {
        let stem = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let prefix = stem.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
        let number = stem[prefix.len()..].parse::<u64>().ok();
        (prefix, number, stem)
    });
    Ok(files)
}

/// Split an optional `<label>=` prefix (letters, digits, `_`, `-`) off a node command
pub fn split_label(cmd: &str) -> (Option<&str>, &str) {
    match cmd.split_once('=') {
        Some((label, rest))
            if !label.is_empty()
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') =>
        {
            (Some(label), rest)
        }
        _ => (None, cmd),
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

    #[test]
    fn split_label_only_takes_a_plain_prefix() {
        assert_eq!(split_label("polkajam=polkajam run {i}"), (Some("polkajam"), "polkajam run {i}"));
        assert_eq!(split_label("node-2_b=./n --x=1"), (Some("node-2_b"), "./n --x=1"));
        assert_eq!(split_label("./node --port=4000{i}"), (None, "./node --port=4000{i}"));
        assert_eq!(split_label("cd x && a=1 ./n"), (None, "cd x && a=1 ./n"));
        assert_eq!(split_label("=./n"), (None, "=./n"));
    }

    #[test]
    fn find_log_files_sorts_naturally() {
        let dir = std::env::temp_dir().join(format!("jam-ui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["node_10.log", "node_2.log", "b.log", "a.log", "notes.txt"] {
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
