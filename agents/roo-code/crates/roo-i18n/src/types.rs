//! Locale type definition.
//!
//! Covers all locales available in the Roo Code i18n system.

use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Locale
// ---------------------------------------------------------------------------

/// Supported locales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    En,
    ZhCn,
    ZhTw,
    Ja,
    Ko,
    De,
    Fr,
    Es,
    It,
    Pt,
    Ru,
    Ar,
    Hi,
    Th,
    Vi,
    Pl,
    Nl,
    Tr,
}

impl Locale {
    /// Return all available locales.
    pub fn all() -> &'static [Locale] {
        &[
            Locale::En,
            Locale::ZhCn,
            Locale::ZhTw,
            Locale::Ja,
            Locale::Ko,
            Locale::De,
            Locale::Fr,
            Locale::Es,
            Locale::It,
            Locale::Pt,
            Locale::Ru,
            Locale::Ar,
            Locale::Hi,
            Locale::Th,
            Locale::Vi,
            Locale::Pl,
            Locale::Nl,
            Locale::Tr,
        ]
    }

    /// Return the default locale (English).
    pub fn default_locale() -> Locale {
        Locale::En
    }

    /// Return the standard locale code string (e.g. `"en"`, `"zh-CN"`).
    pub fn code(&self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::ZhCn => "zh-CN",
            Locale::ZhTw => "zh-TW",
            Locale::Ja => "ja",
            Locale::Ko => "ko",
            Locale::De => "de",
            Locale::Fr => "fr",
            Locale::Es => "es",
            Locale::It => "it",
            Locale::Pt => "pt",
            Locale::Ru => "ru",
            Locale::Ar => "ar",
            Locale::Hi => "hi",
            Locale::Th => "th",
            Locale::Vi => "vi",
            Locale::Pl => "pl",
            Locale::Nl => "nl",
            Locale::Tr => "tr",
        }
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl FromStr for Locale {
    type Err = LocaleParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "en" => Ok(Locale::En),
            "zh-CN" | "zh_cn" | "zhcn" => Ok(Locale::ZhCn),
            "zh-TW" | "zh_tw" | "zhtw" => Ok(Locale::ZhTw),
            "ja" => Ok(Locale::Ja),
            "ko" => Ok(Locale::Ko),
            "de" => Ok(Locale::De),
            "fr" => Ok(Locale::Fr),
            "es" => Ok(Locale::Es),
            "it" => Ok(Locale::It),
            "pt" | "pt-BR" => Ok(Locale::Pt),
            "ru" => Ok(Locale::Ru),
            "ar" => Ok(Locale::Ar),
            "hi" => Ok(Locale::Hi),
            "th" => Ok(Locale::Th),
            "vi" => Ok(Locale::Vi),
            "pl" => Ok(Locale::Pl),
            "nl" => Ok(Locale::Nl),
            "tr" => Ok(Locale::Tr),
            _ => Err(LocaleParseError(s.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Error returned when a locale string cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown locale: {_0}")]
pub struct LocaleParseError(String);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_from_str_en() {
        assert_eq!("en".parse::<Locale>(), Ok(Locale::En));
    }

    #[test]
    fn test_locale_from_str_zh_cn() {
        assert_eq!("zh-CN".parse::<Locale>(), Ok(Locale::ZhCn));
        assert_eq!("zh_cn".parse::<Locale>(), Ok(Locale::ZhCn));
    }

    #[test]
    fn test_locale_from_str_ja() {
        assert_eq!("ja".parse::<Locale>(), Ok(Locale::Ja));
    }

    #[test]
    fn test_locale_from_str_pt_br() {
        assert_eq!("pt-BR".parse::<Locale>(), Ok(Locale::Pt));
    }

    #[test]
    fn test_locale_from_str_invalid() {
        assert!("xx".parse::<Locale>().is_err());
        assert!("".parse::<Locale>().is_err());
    }

    #[test]
    fn test_locale_display() {
        assert_eq!(Locale::En.to_string(), "en");
        assert_eq!(Locale::ZhCn.to_string(), "zh-CN");
        assert_eq!(Locale::Ja.to_string(), "ja");
    }

    #[test]
    fn test_locale_display_all() {
        for &locale in Locale::all() {
            let code = locale.to_string();
            assert!(!code.is_empty(), "locale {locale:?} has empty code");
        }
    }

    #[test]
    fn test_default_locale_is_en() {
        assert_eq!(Locale::default_locale(), Locale::En);
    }

    #[test]
    fn test_available_locales() {
        let all = Locale::all();
        assert!(all.len() >= 18);
        assert!(all.contains(&Locale::En));
        assert!(all.contains(&Locale::ZhCn));
    }

    #[test]
    fn test_locale_roundtrip() {
        for &locale in Locale::all() {
            let code = locale.code();
            let parsed: Locale = code.parse().unwrap();
            assert_eq!(parsed, locale, "roundtrip failed for {locale:?}");
        }
    }
}
