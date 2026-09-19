use anyhow::{anyhow, bail, Result};

const DEFAULT_URL: &str = "http://127.0.0.1:4096";
const HELP: &str = "opencode-tui --url <base-url> [--dir <path>]";

/// Parsed CLI arguments. Mirrors the upstream `context/args` surface for the
/// options M1 supports: the server URL and the working-directory header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Args {
    pub url: String,
    pub directory: Option<String>,
}

impl Args {
    pub fn from_env() -> Result<Self> {
        let argv: Vec<String> = std::env::args().skip(1).collect();
        if argv.iter().any(|arg| arg == "-h" || arg == "--help") {
            println!("{HELP}");
            std::process::exit(0);
        }
        Self::parse(
            argv,
            std::env::var("OPENCODE_URL").ok(),
            std::env::var("OPENCODE_DIRECTORY").ok(),
        )
    }

    /// Pure parse of argv and the environment defaults, so the behavior is
    /// testable without mutating process state.
    fn parse(
        argv: impl IntoIterator<Item = String>,
        url_env: Option<String>,
        directory_env: Option<String>,
    ) -> Result<Self> {
        let mut url = url_env.unwrap_or_else(|| DEFAULT_URL.to_string());
        let mut directory = directory_env;

        let mut argv = argv.into_iter();
        while let Some(arg) = argv.next() {
            match arg.as_str() {
                "--url" => url = argv.next().ok_or_else(|| anyhow!("--url needs a value"))?,
                "--dir" | "--directory" => {
                    directory = Some(argv.next().ok_or_else(|| anyhow!("--dir needs a value"))?)
                }
                "-h" | "--help" => unreachable!("help is handled before parse"),
                other => bail!("unknown argument: {other}"),
            }
        }

        Ok(Self { url, directory })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(argv: &[&str], url_env: Option<&str>, dir_env: Option<&str>) -> Result<Args> {
        Args::parse(
            argv.iter().map(|arg| arg.to_string()),
            url_env.map(str::to_string),
            dir_env.map(str::to_string),
        )
    }

    #[test]
    fn defaults_to_the_local_server_without_a_directory() {
        assert_eq!(
            parse(&[], None, None).unwrap(),
            Args {
                url: DEFAULT_URL.to_string(),
                directory: None,
            }
        );
    }

    #[test]
    fn environment_supplies_defaults() {
        let args = parse(&[], Some("http://env:1"), Some("/env/dir")).unwrap();
        assert_eq!(args.url, "http://env:1");
        assert_eq!(args.directory.as_deref(), Some("/env/dir"));
    }

    #[test]
    fn flags_override_the_environment() {
        let args = parse(
            &["--url", "http://flag:2", "--dir", "/flag/dir"],
            Some("http://env:1"),
            Some("/env/dir"),
        )
        .unwrap();
        assert_eq!(args.url, "http://flag:2");
        assert_eq!(args.directory.as_deref(), Some("/flag/dir"));
    }

    #[test]
    fn directory_alias_is_accepted() {
        let args = parse(&["--directory", "/alias"], None, None).unwrap();
        assert_eq!(args.directory.as_deref(), Some("/alias"));
    }

    #[test]
    fn missing_values_and_unknown_flags_are_errors() {
        assert!(parse(&["--url"], None, None).is_err());
        assert!(parse(&["--dir"], None, None).is_err());
        assert!(parse(&["--nope"], None, None).is_err());
    }
}
