// SPDX-License-Identifier: Apache-2.0

//! Optional JSON config at `$XDG_CONFIG_HOME/pouletb/config.json`
//! (default `~/.config/pouletb/config.json`). A missing file means defaults;
//! a malformed one is an error rather than being silently ignored.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Default download directory; `~` is expanded. `-o` overrides it.
    pub output: Option<String>,
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text)
                .with_context(|| format!("invalid config {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    pub fn output_dir(&self) -> Option<PathBuf> {
        self.output.as_deref().map(expand_tilde)
    }
}

pub fn default_path() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => std::env::home_dir()?.join(".config"),
    };
    Some(base.join("pouletb").join("config.json"))
}

fn expand_tilde(path: &str) -> PathBuf {
    let home = std::env::home_dir();
    match (path.strip_prefix('~'), home) {
        (Some(""), Some(home)) => home,
        (Some(rest), Some(home)) if rest.starts_with('/') => home.join(&rest[1..]),
        _ => PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expands_to_home() {
        let home = std::env::home_dir().unwrap();
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/Public"), home.join("Public"));
        assert_eq!(expand_tilde("/tmp/x"), PathBuf::from("/tmp/x"));
        assert_eq!(expand_tilde("~other/x"), PathBuf::from("~other/x"));
    }

    #[test]
    fn parses_output_and_rejects_unknown_keys() {
        let c: Config = serde_json::from_str(r#"{"output": "~/Public"}"#).unwrap();
        assert_eq!(c.output_dir(), Some(std::env::home_dir().unwrap().join("Public")));
        assert!(serde_json::from_str::<Config>(r#"{"ouput": "x"}"#).is_err());
    }

    #[test]
    fn missing_file_is_default() {
        let c = Config::load(Path::new("/nonexistent/pouletb.json")).unwrap();
        assert!(c.output.is_none());
    }
}
