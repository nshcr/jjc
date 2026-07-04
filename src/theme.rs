use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use serde::Deserialize;

use crate::syntax::HighlightClass;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Theme {
    comment: TextStyle,
    function: TextStyle,
    keyword: TextStyle,
    number: TextStyle,
    string: TextStyle,
    type_name: TextStyle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextStyle {
    fg: Option<Color>,
    bold: bool,
    dim: bool,
    italic: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct ThemeConfig {
    pub comment: Option<TextStyleConfig>,
    pub function: Option<TextStyleConfig>,
    pub keyword: Option<TextStyleConfig>,
    pub number: Option<TextStyleConfig>,
    pub string: Option<TextStyleConfig>,
    pub type_name: Option<TextStyleConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct TextStyleConfig {
    pub fg: Option<String>,
    pub bold: Option<bool>,
    pub dim: Option<bool>,
    pub italic: Option<bool>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            comment: TextStyle::new(Some(Color::DarkGray), false, true, false),
            function: TextStyle::new(Some(Color::Yellow), false, false, false),
            keyword: TextStyle::new(Some(Color::Cyan), true, false, false),
            number: TextStyle::new(Some(Color::Magenta), false, false, false),
            string: TextStyle::new(Some(Color::Green), false, false, false),
            type_name: TextStyle::new(Some(Color::Blue), false, false, false),
        }
    }
}

impl Theme {
    pub fn from_config(config: &ThemeConfig) -> Result<Self, String> {
        let mut theme = Self::default();
        theme.comment.apply(config.comment.as_ref())?;
        theme.function.apply(config.function.as_ref())?;
        theme.keyword.apply(config.keyword.as_ref())?;
        theme.number.apply(config.number.as_ref())?;
        theme.string.apply(config.string.as_ref())?;
        theme.type_name.apply(config.type_name.as_ref())?;
        Ok(theme)
    }

    pub fn style(&self, class: HighlightClass) -> Style {
        match class {
            HighlightClass::Comment => self.comment.to_style(),
            HighlightClass::Function => self.function.to_style(),
            HighlightClass::Keyword => self.keyword.to_style(),
            HighlightClass::Number => self.number.to_style(),
            HighlightClass::String => self.string.to_style(),
            HighlightClass::Type => self.type_name.to_style(),
        }
    }
}

impl TextStyle {
    fn new(fg: Option<Color>, bold: bool, dim: bool, italic: bool) -> Self {
        Self {
            fg,
            bold,
            dim,
            italic,
        }
    }

    fn apply(&mut self, config: Option<&TextStyleConfig>) -> Result<(), String> {
        let Some(config) = config else {
            return Ok(());
        };
        if let Some(fg) = &config.fg {
            self.fg = Some(parse_color(fg)?);
        }
        if let Some(bold) = config.bold {
            self.bold = bold;
        }
        if let Some(dim) = config.dim {
            self.dim = dim;
        }
        if let Some(italic) = config.italic {
            self.italic = italic;
        }
        Ok(())
    }

    fn to_style(&self) -> Style {
        let mut style = Style::new();
        if let Some(fg) = self.fg {
            style = style.fg(fg);
        }
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.dim {
            style = style.add_modifier(Modifier::DIM);
        }
        if self.italic {
            style = style.add_modifier(Modifier::ITALIC);
        }
        style
    }
}

fn parse_color(value: &str) -> Result<Color, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "gray" | "grey" => Ok(Color::Gray),
        "dark-gray" | "dark-grey" => Ok(Color::DarkGray),
        "white" => Ok(Color::White),
        value if value.starts_with('#') && value.len() == 7 => {
            let red = u8::from_str_radix(&value[1..3], 16).map_err(|_| bad_color(value))?;
            let green = u8::from_str_radix(&value[3..5], 16).map_err(|_| bad_color(value))?;
            let blue = u8::from_str_radix(&value[5..7], 16).map_err(|_| bad_color(value))?;
            Ok(Color::Rgb(red, green, blue))
        }
        _ => Err(bad_color(value)),
    }
}

fn bad_color(value: &str) -> String {
    format!("unsupported color {value:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_theme_overrides() {
        let config = ThemeConfig {
            keyword: Some(TextStyleConfig {
                fg: Some("#112233".into()),
                bold: Some(false),
                dim: Some(true),
                italic: Some(true),
            }),
            ..ThemeConfig::default()
        };

        let theme = Theme::from_config(&config).unwrap();

        assert_eq!(
            theme.keyword,
            TextStyle::new(Some(Color::Rgb(0x11, 0x22, 0x33)), false, true, true)
        );
    }

    #[test]
    fn rejects_unknown_color() {
        let config = ThemeConfig {
            string: Some(TextStyleConfig {
                fg: Some("blurple".into()),
                ..TextStyleConfig::default()
            }),
            ..ThemeConfig::default()
        };

        assert!(Theme::from_config(&config).is_err());
    }
}
