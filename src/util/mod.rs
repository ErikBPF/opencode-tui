/// Collapse a `$HOME` prefix to `~`, matching the upstream path formatter.
pub fn abbreviate_home(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => abbreviate_with(&home, path),
        _ => path.to_string(),
    }
}

/// Replace `home` only on a path boundary, so `/home/erik2` is not abbreviated
/// by a `/home/erik` home.
fn abbreviate_with(home: &str, path: &str) -> String {
    let boundary = matches!(path.as_bytes().get(home.len()), None | Some(b'/'));
    if path.starts_with(home) && boundary {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbreviates_on_boundary_only() {
        let home = "/home/erik";
        assert_eq!(abbreviate_with(home, "/home/erik/project"), "~/project");
        assert_eq!(abbreviate_with(home, "/home/erik"), "~");
        assert_eq!(
            abbreviate_with(home, "/home/erik2/project"),
            "/home/erik2/project"
        );
        assert_eq!(abbreviate_with(home, "/srv/data"), "/srv/data");
    }
}
