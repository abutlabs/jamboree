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

/// Split an optional `<label>=` prefix (letters, digits, `_`, `-`) off a node command. An
/// all-caps prefix such as `RUST_LOG=debug` is an environment variable, not a label
pub fn split_label(cmd: &str) -> (Option<&str>, &str) {
    match cmd.split_once('=') {
        Some((label, rest))
            if label.chars().any(|c| c.is_ascii_lowercase())
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') =>
        {
            (Some(label), rest)
        }
        _ => (None, cmd),
    }
}

/// Each `<label>*K=` command as K copies of `<label>=`; others unchanged
pub fn repeat_counts(cmds: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for cmd in cmds {
        let repeat = (cmd.split_once('='))
            .and_then(|(prefix, rest)| Some((prefix.rsplit_once('*')?, rest)))
            .filter(|((label, _), rest)| split_label(&format!("{label}={rest}")).0.is_some());
        match repeat {
            Some(((label, count), rest)) => {
                let count: usize = (count.parse().ok().filter(|&k| k > 0))
                    .ok_or(format!("--cmd '{label}*{count}=': the count must be a number above 0"))?;
                out.extend(std::iter::repeat_n(format!("{label}={rest}"), count));
            }
            None => out.push(cmd.clone()),
        }
    }
    Ok(out)
}

/// Fail on a `{word}` that is not a jamboree placeholder, such as a misspelt `{prot}`; shell
/// `${VAR}` is left alone
pub fn check_placeholders(cmd: &str) -> Result<(), String> {
    static WORD: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(^|[^$])\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap()
    });
    let known = ["i", "port", "container", "telemetry"];
    match WORD.captures_iter(cmd).map(|c| c[2].to_string()).find(|w| !known.contains(&w.as_str())) {
        Some(word) => Err(format!(
            "--cmd has an unknown placeholder {{{word}}}; known: {{i}} {{port}} {{container}} {{telemetry}}"
        )),
        None => Ok(()),
    }
}

/// Next number from splitmix64 state, the same on every machine and version of Rust
pub fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
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
    fn repeat_counts_copies_a_counted_label() {
        let cmds = ["pj*2=pj {i}".to_string(), "mine=./n --x=1*2".to_string()];
        assert_eq!(repeat_counts(&cmds).unwrap(), ["pj=pj {i}", "pj=pj {i}", "mine=./n --x=1*2"]);
        let plain = ["./n --a*2=b".to_string(), "RUST_LOG*2=x".to_string()];
        assert_eq!(repeat_counts(&plain).unwrap(), plain);
        assert!(repeat_counts(&["pj*0=pj".to_string()]).is_err());
        assert!(repeat_counts(&["pj*x=pj".to_string()]).is_err());
    }

    #[test]
    fn check_placeholders_names_an_unknown_one() {
        let ok = "./n {i} --port {port} --telemetry {telemetry} ${HOME} {a,b} awk '{print $1}'";
        assert_eq!(check_placeholders(ok), Ok(()));
        let err = check_placeholders("./n --port {prot}").unwrap_err();
        assert!(err.contains("{prot}"), "{err}");
        assert!(check_placeholders("{I}").is_err());
    }

    #[test]
    fn split_label_only_takes_a_plain_prefix() {
        assert_eq!(split_label("polkajam=polkajam run {i}"), (Some("polkajam"), "polkajam run {i}"));
        assert_eq!(split_label("node-2_b=./n --x=1"), (Some("node-2_b"), "./n --x=1"));
        assert_eq!(split_label("./node --port=4000{i}"), (None, "./node --port=4000{i}"));
        assert_eq!(split_label("cd x && a=1 ./n"), (None, "cd x && a=1 ./n"));
        assert_eq!(split_label("=./n"), (None, "=./n"));
        assert_eq!(split_label("RUST_LOG=debug ./n"), (None, "RUST_LOG=debug ./n"));
        assert_eq!(split_label("pj2=RUST_LOG=debug ./n"), (Some("pj2"), "RUST_LOG=debug ./n"));
    }

    #[test]
    fn find_log_files_sorts_naturally() {
        let dir = std::env::temp_dir().join(format!("jamboree-test-{}", std::process::id()));
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
