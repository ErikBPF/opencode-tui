/// Collapse a `$HOME` prefix to `~`, matching the upstream path formatter.
pub fn abbreviate_home(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.starts_with(&home) => path.replacen(&home, "~", 1),
        _ => path.to_string(),
    }
}
