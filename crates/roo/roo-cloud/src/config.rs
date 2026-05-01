/// Cloud service configuration constants and helpers.
/// Mirrors packages/cloud/src/config.ts

/// Production Clerk base URL for authentication.
pub const PRODUCTION_CLERK_BASE_URL: &str = "https://clerk.roocode.com";

/// Production Roo Code API URL.
pub const PRODUCTION_ROO_CODE_API_URL: &str = "https://app.roocode.com";

/// Returns the Clerk base URL, checking the `CLERK_BASE_URL` environment variable first.
pub fn get_clerk_base_url() -> String {
    std::env::var("CLERK_BASE_URL").unwrap_or_else(|_| PRODUCTION_CLERK_BASE_URL.to_string())
}

/// Returns the Roo Code API URL, checking the `ROO_CODE_API_URL` environment variable first.
pub fn get_roo_code_api_url() -> String {
    std::env::var("ROO_CODE_API_URL").unwrap_or_else(|_| PRODUCTION_ROO_CODE_API_URL.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_clerk_url() {
        // When env var is not set, should return production URL
        let url = get_clerk_base_url();
        assert!(!url.is_empty());
    }

    #[test]
    fn test_default_api_url() {
        let url = get_roo_code_api_url();
        assert!(!url.is_empty());
    }

    #[test]
    fn test_production_constants() {
        assert_eq!("https://clerk.roocode.com", PRODUCTION_CLERK_BASE_URL);
        assert_eq!("https://app.roocode.com", PRODUCTION_ROO_CODE_API_URL);
    }
}
