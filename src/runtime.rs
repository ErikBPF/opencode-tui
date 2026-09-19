use anyhow::{anyhow, bail, Result};

/// Parsed CLI arguments. Mirrors the upstream `context/args` surface for the
/// options M1 supports: the server URL and the working-directory header.
#[derive(Clone, Debug)]
pub struct Args {
    pub url: String,
    pub directory: Option<String>,
}

impl Args {
    pub fn from_env() -> Result<Self> {
        let mut url =
            std::env::var("OPENCODE_URL").unwrap_or_else(|_| "http://127.0.0.1:4096".to_string());
        let mut directory = std::env::var("OPENCODE_DIRECTORY").ok();

        let mut argv = std::env::args().skip(1);
        while let Some(arg) = argv.next() {
            match arg.as_str() {
                "--url" => url = argv.next().ok_or_else(|| anyhow!("--url needs a value"))?,
                "--dir" | "--directory" => {
                    directory = Some(argv.next().ok_or_else(|| anyhow!("--dir needs a value"))?)
                }
                "-h" | "--help" => {
                    println!("opencode-tui --url <base-url> [--dir <path>]");
                    std::process::exit(0);
                }
                other => bail!("unknown argument: {other}"),
            }
        }

        Ok(Self { url, directory })
    }
}
