//! AWS SigV4 signing for Bedrock API requests.
//!
//! Simplified SigV4 implementation for signing requests to AWS Bedrock.
//! Uses HMAC-SHA256 for signing.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// AWS SigV4 signer for Bedrock requests.
pub struct SigV4Signer {
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
    region: String,
    service: String,
}

impl SigV4Signer {
    /// Create a new SigV4 signer.
    pub fn new(
        access_key: String,
        secret_key: String,
        session_token: Option<String>,
        region: String,
    ) -> Self {
        Self {
            access_key,
            secret_key,
            session_token,
            region,
            service: "bedrock".to_string(),
        }
    }

    /// Sign a request and return the Authorization header value.
    pub fn sign(
        &self,
        method: &str,
        url: &str,
        body: &[u8],
        timestamp: &chrono::DateTime<chrono::Utc>,
    ) -> String {
        let date_stamp = timestamp.format("%Y%m%d").to_string();
        let amz_date = timestamp.format("%Y%m%dT%H%M%SZ").to_string();

        // Parse host and path from URL
        let (host, path, query) = parse_url(url);

        // Step 1: Create canonical request
        let payload_hash = hex_encode(&Sha256::digest(body));
        let canonical_headers = format!(
            "content-type:application/json\nhost:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
            host, payload_hash, amz_date
        );
        let signed_headers =
            "content-type;host;x-amz-content-sha256;x-amz-date".to_string();

        let canonical_querystring = query.as_deref().unwrap_or("");

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method,
            path,
            canonical_querystring,
            canonical_headers,
            signed_headers,
            payload_hash
        );

        // Step 2: Create string to sign
        let credential_scope = format!(
            "{}/{}/{}/aws4_request",
            date_stamp, self.region, self.service
        );
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date,
            credential_scope,
            hex_encode(&Sha256::digest(canonical_request.as_bytes()))
        );

        // Step 3: Calculate signature
        let signing_key = get_signature_key(
            &self.secret_key,
            &date_stamp,
            &self.region,
            &self.service,
        );
        let signature = hmac_sha256(&signing_key, string_to_sign.as_bytes());
        let signature_hex = hex_encode(&signature);

        // Step 4: Build authorization header
        format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.access_key, credential_scope, signed_headers, signature_hex
        )
    }

    /// Get the X-Amz-Date header value.
    pub fn amz_date(&self, timestamp: &chrono::DateTime<chrono::Utc>) -> String {
        timestamp.format("%Y%m%dT%H%M%SZ").to_string()
    }

    /// Get the session token if present.
    pub fn session_token(&self) -> Option<&str> {
        self.session_token.as_deref()
    }

    /// Get the region as a string slice.
    pub fn region_str(&self) -> &str {
        &self.region
    }
}

fn parse_url(url: &str) -> (String, String, Option<String>) {
    // Simple URL parsing
    let without_scheme = url.strip_prefix("https://").unwrap_or(url);
    let parts: Vec<&str> = without_scheme.splitn(2, '/').collect();
    let host = parts[0].to_string();
    let rest = parts.get(1).unwrap_or(&"");

    let (path, query) = if let Some(pos) = rest.find('?') {
        (rest[..pos].to_string(), Some(rest[pos + 1..].to_string()))
    } else {
        (rest.to_string(), None)
    };

    let path = if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", path)
    };

    (host, path, query)
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn get_signature_key(
    secret_key: &str,
    date_stamp: &str,
    region: &str,
    service: &str,
) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{}", secret_key).as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signer_creates_authorization() {
        let signer = SigV4Signer::new(
            "AKIAIOSFODNN7EXAMPLE".to_string(),
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            None,
            "us-east-1".to_string(),
        );

        let timestamp = chrono::Utc::now();
        let auth = signer.sign(
            "POST",
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/test/converse",
            b"{}",
            &timestamp,
        );

        assert!(auth.starts_with("AWS4-HMAC-SHA256"));
        assert!(auth.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(auth.contains("us-east-1"));
        assert!(auth.contains("bedrock"));
    }

    #[test]
    fn test_amz_date_format() {
        let signer = SigV4Signer::new(
            "test".to_string(),
            "test".to_string(),
            None,
            "us-east-1".to_string(),
        );

        let timestamp = chrono::DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let amz_date = signer.amz_date(&timestamp);
        assert_eq!(amz_date, "20240115T103000Z");
    }

    #[test]
    fn test_session_token() {
        let signer = SigV4Signer::new(
            "test".to_string(),
            "test".to_string(),
            Some("session-token-123".to_string()),
            "us-east-1".to_string(),
        );
        assert_eq!(signer.session_token(), Some("session-token-123"));
    }

    #[test]
    fn test_no_session_token() {
        let signer = SigV4Signer::new(
            "test".to_string(),
            "test".to_string(),
            None,
            "us-east-1".to_string(),
        );
        assert!(signer.session_token().is_none());
    }

    #[test]
    fn test_parse_url() {
        let (host, path, query) =
            parse_url("https://bedrock-runtime.us-east-1.amazonaws.com/model/test/converse");
        assert_eq!(host, "bedrock-runtime.us-east-1.amazonaws.com");
        assert_eq!(path, "/model/test/converse");
        assert!(query.is_none());
    }

    #[test]
    fn test_parse_url_with_query() {
        let (host, path, query) = parse_url(
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/test?foo=bar",
        );
        assert_eq!(host, "bedrock-runtime.us-east-1.amazonaws.com");
        assert_eq!(path, "/model/test");
        assert_eq!(query, Some("foo=bar".to_string()));
    }

    #[test]
    fn test_hex_encode() {
        let bytes = vec![0x01, 0x23, 0xab, 0xcd];
        assert_eq!(hex_encode(&bytes), "0123abcd");
    }

    #[test]
    fn test_hmac_sha256_deterministic() {
        let key = b"test-key";
        let data = b"test-data";
        let result1 = hmac_sha256(key, data);
        let result2 = hmac_sha256(key, data);
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_get_signature_key_deterministic() {
        let key1 = get_signature_key("secret", "20240115", "us-east-1", "bedrock");
        let key2 = get_signature_key("secret", "20240115", "us-east-1", "bedrock");
        assert_eq!(key1, key2);
    }
}
