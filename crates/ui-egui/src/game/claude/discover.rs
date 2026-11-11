//! Finding the programs the chat runs: the Claude Code CLI and `figcraft-cli` (the MCP server that
//! lets Claude drive this window). Desktop only.

use std::path::{Path, PathBuf};

/// The first file named one of `names` in one of `dirs`.
pub fn locate(names: &[&str], dirs: impl IntoIterator<Item = PathBuf>, is_file: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    dirs.into_iter().find_map(|dir| names.iter().map(|n| dir.join(n)).find(|p| is_file(p)))
}

/// The folders of the `PATH` variable.
pub fn path_dirs(path: Option<std::ffi::OsString>) -> Vec<PathBuf> {
    path.map(|p| std::env::split_paths(&p).filter(|d| !d.as_os_str().is_empty()).collect()).unwrap_or_default()
}

/// Where the Claude Code installer puts `claude`, after `PATH`.
pub fn claude_homes(home: Option<PathBuf>) -> Vec<PathBuf> {
    let Some(home) = home else { return Vec::new() };
    vec![home.join(".local").join("bin"), home.join(".claude").join("local")]
}

// Only the native program: a `.cmd` wrapper (an npm install) cannot take the guide prompt as an argument
// (Windows batch files refuse newlines and quotes in arguments).
const CLAUDE_NAMES: &[&str] = if cfg!(windows) { &["claude.exe"] } else { &["claude"] };
const CLI_NAMES: &[&str] = if cfg!(windows) { &["figcraft-cli.exe"] } else { &["figcraft-cli"] };

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// The Claude Code CLI: `PATH` first, then where its installer puts it.
pub fn find_claude() -> Option<PathBuf> {
    let dirs = path_dirs(std::env::var_os("PATH")).into_iter().chain(claude_homes(home()));
    locate(CLAUDE_NAMES, dirs, Path::is_file)
}

/// `figcraft-cli`: next to the running program (the installed copy ships it there), else `PATH`.
pub fn find_cli() -> Option<PathBuf> {
    let beside = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf));
    let dirs = beside.into_iter().chain(path_dirs(std::env::var_os("PATH")));
    locate(CLI_NAMES, dirs, Path::is_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_folder_with_the_file_wins() {
        let dirs = vec![PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")];
        let has = |p: &Path| p == Path::new("b").join("claude") || p == Path::new("c").join("claude");
        assert_eq!(locate(&["claude"], dirs.clone(), has), Some(PathBuf::from("b").join("claude")));
        assert_eq!(locate(&["claude"], dirs.clone(), |_| false), None);
        assert_eq!(locate(&["x", "claude"], dirs, has), Some(PathBuf::from("b").join("claude")));
        assert_eq!(locate(&["claude"], Vec::new(), |_| true), None);
    }

    #[test]
    fn path_is_split_and_empty_entries_dropped() {
        let joined = std::env::join_paths([PathBuf::from("x"), PathBuf::from("y")]).unwrap_or_default();
        assert_eq!(path_dirs(Some(joined)), vec![PathBuf::from("x"), PathBuf::from("y")]);
        assert!(path_dirs(None).is_empty());
        assert!(path_dirs(Some(std::ffi::OsString::new())).is_empty());
    }

    #[test]
    fn claude_is_looked_for_in_the_installer_folder() {
        let homes = claude_homes(Some(PathBuf::from("h")));
        assert_eq!(homes[0], PathBuf::from("h").join(".local").join("bin"));
        assert!(claude_homes(None).is_empty());
    }

    #[test]
    fn cli_is_found_beside_a_given_folder() {
        let dir = std::env::temp_dir().join(format!("graybox-discover-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let name = CLI_NAMES[0];
        assert_eq!(locate(CLI_NAMES, vec![dir.clone()], Path::is_file), None);
        std::fs::write(dir.join(name), b"").unwrap();
        assert_eq!(locate(CLI_NAMES, vec![dir.clone()], Path::is_file), Some(dir.join(name)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
