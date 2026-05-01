/// Cloud utility functions.
/// Mirrors packages/cloud/src/utils.ts

/// Returns the User-Agent header value for cloud requests.
pub fn get_user_agent(version: Option<&str>) -> String {
    let ver = version.unwrap_or("unknown");
    format!("Roo-Code {}", ver)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_user_agent_with_version() {
        let ua = get_user_agent(Some("1.0.0"));
        assert_eq!("Roo-Code 1.0.0", ua);
    }

    #[test]
    fn test_get_user_agent_without_version() {
        let ua = get_user_agent(None);
        assert_eq!("Roo-Code unknown", ua);
    }
}
