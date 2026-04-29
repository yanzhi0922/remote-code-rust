//! Translation loader / [`I18n`] facade.
//!
//! Provides the main [`I18n`] struct that holds the current locale and its
//! translation map, mirroring the `i18next` API used in the TypeScript
//! source (`t()`, `changeLanguage()`, …).

use std::collections::HashMap;

use crate::translations;
use crate::types::Locale;

// ---------------------------------------------------------------------------
// I18n
// ---------------------------------------------------------------------------

/// Internationalisation manager.
///
/// Holds the current locale and a flat key → value translation map.
/// Unknown keys fall back to the key itself.
pub struct I18n {
    locale: Locale,
    translations: HashMap<String, String>,
}

impl I18n {
    /// Create a new instance for the given locale.
    pub fn new(locale: Locale) -> Self {
        let translations = translations::get_translations(locale);
        Self { locale, translations }
    }

    /// Translate `key`, returning the key itself when no translation exists.
    pub fn t(&self, key: &str) -> String {
        self.translations
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    /// Translate `key` with `{{param}}`-style interpolation.
    ///
    /// Each entry in `args` replaces `{{key}}` in the translated string.
    pub fn t_with_args(&self, key: &str, args: &HashMap<&str, &str>) -> String {
        let mut text = self.t(key);
        for (&k, &v) in args {
            let placeholder = format!("{{{{{k}}}}}");
            text = text.replace(&placeholder, v);
        }
        text
    }

    /// Switch to a different locale, reloading translations.
    pub fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
        self.translations = translations::get_translations(locale);
    }

    /// Return the current locale.
    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// Return all supported locales.
    pub fn available_locales() -> Vec<Locale> {
        Locale::all().to_vec()
    }

    /// Return the default locale.
    pub fn default_locale() -> Locale {
        Locale::default_locale()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Construction ----

    #[test]
    fn test_i18n_new_en() {
        let i18n = I18n::new(Locale::En);
        assert_eq!(i18n.locale(), Locale::En);
    }

    #[test]
    fn test_i18n_new_zh_cn() {
        let i18n = I18n::new(Locale::ZhCn);
        assert_eq!(i18n.locale(), Locale::ZhCn);
    }

    // ---- t() English ----

    #[test]
    fn test_translation_en_save() {
        let i18n = I18n::new(Locale::En);
        assert_eq!(i18n.t("save"), "Save");
    }

    #[test]
    fn test_translation_en_cancel() {
        let i18n = I18n::new(Locale::En);
        assert_eq!(i18n.t("cancel"), "Cancel");
    }

    #[test]
    fn test_translation_en_error() {
        let i18n = I18n::new(Locale::En);
        assert_eq!(i18n.t("error"), "Error");
    }

    #[test]
    fn test_translation_en_all_common_keys() {
        let i18n = I18n::new(Locale::En);
        for key in [
            "save", "cancel", "error", "warning", "loading", "yes", "no",
            "ok", "retry", "close", "settings", "help", "about", "version",
            "language", "theme", "mode", "tool", "file", "folder", "search",
            "edit", "delete", "create", "update", "run", "stop", "start",
            "pause", "resume", "reset", "clear", "refresh",
        ] {
            let val = i18n.t(key);
            assert_ne!(val, key, "en translation missing for key: {key}");
        }
    }

    // ---- t() Chinese ----

    #[test]
    fn test_translation_zh_cn_save() {
        let i18n = I18n::new(Locale::ZhCn);
        assert_eq!(i18n.t("save"), "保存");
    }

    #[test]
    fn test_translation_zh_cn_cancel() {
        let i18n = I18n::new(Locale::ZhCn);
        assert_eq!(i18n.t("cancel"), "取消");
    }

    #[test]
    fn test_translation_zh_cn_all_common_keys() {
        let i18n = I18n::new(Locale::ZhCn);
        for key in [
            "save", "cancel", "error", "warning", "loading", "yes", "no",
            "ok", "retry", "close", "settings", "help", "about", "version",
            "language", "theme", "mode", "tool", "file", "folder", "search",
            "edit", "delete", "create", "update", "run", "stop", "start",
            "pause", "resume", "reset", "clear", "refresh",
        ] {
            let val = i18n.t(key);
            assert_ne!(val, key, "zh-CN translation missing for key: {key}");
        }
    }

    // ---- Fallback ----

    #[test]
    fn test_translation_fallback_to_key() {
        let i18n = I18n::new(Locale::En);
        assert_eq!(i18n.t("nonexistent_key_12345"), "nonexistent_key_12345");
    }

    // ---- t_with_args ----

    #[test]
    fn test_translation_with_args() {
        let i18n = I18n::new(Locale::En);
        let mut args = HashMap::new();
        args.insert("name", "Alice");
        assert_eq!(i18n.t_with_args("welcome", &args), "Welcome, Alice!");
    }

    #[test]
    fn test_translation_with_multiple_args() {
        let i18n = I18n::new(Locale::En);
        let mut args = HashMap::new();
        args.insert("name", "Bob");
        args.insert("count", "5");
        let result = i18n.t_with_args("welcome", &args);
        assert!(result.contains("Bob"));
    }

    #[test]
    fn test_translation_with_args_zh_cn() {
        let i18n = I18n::new(Locale::ZhCn);
        let mut args = HashMap::new();
        args.insert("name", "张三");
        assert_eq!(i18n.t_with_args("welcome", &args), "欢迎，张三！");
    }

    #[test]
    fn test_translation_with_args_items_count() {
        let i18n = I18n::new(Locale::En);
        let mut args = HashMap::new();
        args.insert("count", "42");
        assert_eq!(i18n.t_with_args("items_count", &args), "42 items");
    }

    // ---- set_locale ----

    #[test]
    fn test_set_locale() {
        let mut i18n = I18n::new(Locale::En);
        assert_eq!(i18n.t("save"), "Save");
        i18n.set_locale(Locale::ZhCn);
        assert_eq!(i18n.locale(), Locale::ZhCn);
        assert_eq!(i18n.t("save"), "保存");
    }

    #[test]
    fn test_set_locale_changes_translations() {
        let mut i18n = I18n::new(Locale::En);
        i18n.set_locale(Locale::Ja);
        assert_eq!(i18n.t("save"), "保存");
        assert_eq!(i18n.t("cancel"), "キャンセル");
    }

    // ---- available_locales / default_locale ----

    #[test]
    fn test_available_locales_count() {
        let locales = I18n::available_locales();
        assert!(locales.len() >= 18);
    }

    #[test]
    fn test_default_locale_is_en() {
        assert_eq!(I18n::default_locale(), Locale::En);
    }

    // ---- Other locales have basic keys ----

    #[test]
    fn test_other_locales_have_save() {
        for &locale in Locale::all() {
            let i18n = I18n::new(locale);
            let val = i18n.t("save");
            assert_ne!(
                val,
                "save",
                "locale {locale:?} is missing the 'save' translation"
            );
        }
    }
}
