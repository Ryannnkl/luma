use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, de};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color([u8; 4]);

impl Color {
    #[must_use]
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self([red, green, blue, 255])
    }

    #[must_use]
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self([red, green, blue, alpha])
    }

    #[must_use]
    pub const fn channels(self) -> [u8; 4] {
        self.0
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(de::Error::custom)
    }
}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let hexadecimal = value.strip_prefix('#').ok_or(ParseColorError)?;
        if hexadecimal.len() != 6 && hexadecimal.len() != 8 {
            return Err(ParseColorError);
        }

        let red = parse_channel(&hexadecimal[0..2])?;
        let green = parse_channel(&hexadecimal[2..4])?;
        let blue = parse_channel(&hexadecimal[4..6])?;
        let alpha = if hexadecimal.len() == 8 {
            parse_channel(&hexadecimal[6..8])?
        } else {
            255
        };

        Ok(Self::rgba(red, green, blue, alpha))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ParseColorError;

impl fmt::Display for ParseColorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("expected #RRGGBB or #RRGGBBAA")
    }
}

fn parse_channel(value: &str) -> Result<u8, ParseColorError> {
    u8::from_str_radix(value, 16).map_err(|_| ParseColorError)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::Color;

    #[test]
    fn parses_opaque_color() {
        assert_eq!(Color::from_str("#93e6be"), Ok(Color::rgb(147, 230, 190)));
    }

    #[test]
    fn parses_color_with_alpha() {
        assert_eq!(Color::from_str("#00000052"), Ok(Color::rgba(0, 0, 0, 82)));
    }

    #[test]
    fn rejects_invalid_color() {
        assert!(Color::from_str("93e6be").is_err());
        assert!(Color::from_str("#xyzxyz").is_err());
    }
}
