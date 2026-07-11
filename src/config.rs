use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::Deserialize;

use crate::theme::Theme;
use crate::theme::ThemeConfig;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppConfig {
    pub syntax: SyntaxConfig,
    pub theme: Theme,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, rename_all = "kebab-case")]
pub struct SyntaxConfig {
    pub enabled: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct RawConfig {
    syntax: SyntaxConfig,
    theme: ThemeConfig,
}

impl Default for SyntaxConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl AppConfig {
    pub fn load() -> io::Result<Self> {
        let Some(path) = config_path() else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        Self::parse(&fs::read_to_string(&path)?).map_err(|message| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {message}", path.display()),
            )
        })
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|err| err.to_string())?;
        Ok(Self {
            syntax: raw.syntax,
            theme: Theme::from_config(&raw.theme)?,
        })
    }
}

fn config_path() -> Option<PathBuf> {
    if let Ok(path) = env::var("JJC_CONFIG")
        && !path.is_empty()
    {
        return Some(PathBuf::from(path));
    }
    if let Ok(home) = env::var("XDG_CONFIG_HOME")
        && !home.is_empty()
    {
        return Some(PathBuf::from(home).join("jjc").join("config.toml"));
    }
    env::var("HOME")
        .ok()
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".config/jjc/config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn parses_syntax_and_theme_config() {
        let config = AppConfig::parse(
            r##"
            [syntax]
            enabled = false

            [theme.keyword]
            fg = "#abcdef"
            bold = false
            "##,
        )
        .unwrap();

        assert!(!config.syntax.enabled);
        assert_eq!(
            config
                .theme
                .style(crate::syntax::HighlightClass::Keyword)
                .fg,
            Some(Color::Rgb(0xab, 0xcd, 0xef))
        );
    }
}
