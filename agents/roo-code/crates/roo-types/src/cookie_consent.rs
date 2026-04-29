//! Cookie consent constants and types.
//!
//! Derived from `packages/types/src/cookie-consent.ts`.

/// The name of the cookie that stores user's consent preference.
pub const CONSENT_COOKIE_NAME: &str = "roo-code-cookie-consent";

/// Cookie consent event names.
pub const COOKIE_CONSENT_EVENTS: CookieConsentEvents = CookieConsentEvents {
    CHANGED: "cookieConsentChanged",
};

/// Cookie consent event name constants.
#[allow(non_snake_case)]
pub struct CookieConsentEvents {
    pub CHANGED: &'static str,
}
