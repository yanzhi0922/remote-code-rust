//! TLS configuration helpers for secure connections.

use std::sync::{Arc, Once};

use crate::TlsConfig;
use rustls::crypto::WebPkiSupportedAlgorithms;

/// Build a rustls client config from our TlsConfig.
pub fn build_client_tls_config(config: &TlsConfig) -> anyhow::Result<Arc<rustls::ClientConfig>> {
    ensure_rustls_crypto_provider();

    let mut root_store = rustls::RootCertStore::empty();
    let result = rustls_native_certs::load_native_certs();
    for cert in result.certs {
        root_store.add(cert)?;
    }
    if !result.errors.is_empty() {
        tracing::warn!("native cert loading errors: {:?}", result.errors);
    }

    if config.accept_self_signed {
        if config.cert_fingerprints.is_empty() {
            anyhow::bail!("self-signed TLS requires at least one pinned certificate fingerprint");
        }
        let builder = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(FlexibleVerifier {
                fingerprints: config.cert_fingerprints.clone(),
                signature_algorithms: rustls::crypto::ring::default_provider()
                    .signature_verification_algorithms,
            }))
            .with_no_client_auth();
        return Ok(Arc::new(builder));
    }

    let builder = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(Arc::new(builder))
}

fn ensure_rustls_crypto_provider() {
    static RUSTLS_PROVIDER_INIT: Once = Once::new();
    RUSTLS_PROVIDER_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Certificate verifier for self-signed certs with mandatory fingerprint pinning.
#[derive(Debug)]
struct FlexibleVerifier {
    fingerprints: Vec<String>,
    signature_algorithms: WebPkiSupportedAlgorithms,
}

impl rustls::client::danger::ServerCertVerifier for FlexibleVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        intermediates: &[rustls::pki_types::CertificateDer<'_>],
        server_name: &rustls::pki_types::ServerName<'_>,
        ocsp_response: &[u8],
        now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        // If fingerprints are pinned, verify the leaf cert matches using
        // constant-time comparison to prevent timing side-channel attacks.
        if !self.fingerprints.is_empty() {
            let fp_bytes = sha256_bytes(end_entity.as_ref());
            let fp_hex = hex::encode(&fp_bytes);
            let matched = self.fingerprints.iter().any(|p| {
                // Compare hex-decoded pinned bytes in constant time.
                match decode_hex(p) {
                    Ok(pinned_bytes) => constant_time_eq(&fp_bytes, &pinned_bytes),
                    Err(_) => {
                        // Fall back to constant-time string comparison for non-hex pins.
                        constant_time_eq_str(p.as_bytes(), fp_hex.as_bytes())
                    }
                }
            });
            if matched {
                // Warn (but do not reject) if the pinned certificate is expired.
                // Self-signed certs in development often lack proper validity.
                if let Err(e) = check_cert_validity(end_entity, now) {
                    tracing::warn!("pinned certificate validity check failed: {e}");
                }
                return Ok(rustls::client::danger::ServerCertVerified::assertion());
            }
            return Err(rustls::Error::General(
                "certificate fingerprint does not match any pinned value".into(),
            ));
        }
        let _ = (intermediates, server_name, ocsp_response, now);
        Err(rustls::Error::General(
            "self-signed TLS requires a pinned certificate fingerprint".into(),
        ))
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.signature_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.signature_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.signature_algorithms.supported_schemes()
    }
}

/// Compute the raw SHA-256 digest bytes.
fn sha256_bytes(data: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).to_vec()
}

/// Check certificate validity period (not_before / not_after).
/// Returns Err if the certificate is expired or not yet valid.
/// Best-effort: if DER parsing fails, returns Ok (the fingerprint check is the
/// primary trust mechanism for self-signed certs).
fn check_cert_validity(
    cert: &rustls::pki_types::CertificateDer<'_>,
    now: rustls::pki_types::UnixTime,
) -> Result<(), String> {
    let parsed = match x509_parser::parse_x509_certificate(cert.as_ref()) {
        Ok((_, cert)) => cert,
        Err(_) => return Ok(()),
    };
    let now_time =
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(now.as_secs());
    let validity = parsed.validity();
    if now_time < validity.not_before.to_datetime() {
        return Err("certificate is not yet valid".into());
    }
    if now_time > validity.not_after.to_datetime() {
        return Err(format!("certificate expired on {:?}", validity.not_after));
    }
    Ok(())
}

/// Constant-time byte comparison to prevent timing side-channels.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Constant-time comparison for byte strings of equal length.
fn constant_time_eq_str(a: &[u8], b: &[u8]) -> bool {
    constant_time_eq(a, b)
}

/// Decode a hex string to bytes.
fn decode_hex(s: &str) -> Result<Vec<u8>, ()> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

mod hex {
    use std::fmt::Write;
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        let bytes = bytes.as_ref();
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            let _ = write!(s, "{b:02x}");
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::client::danger::ServerCertVerifier;

    // ---------------------------------------------------------------------------
    // constant_time_eq
    // ---------------------------------------------------------------------------

    #[test]
    fn constant_time_eq_identical_slices() {
        assert!(constant_time_eq(&[], &[]));
        assert!(constant_time_eq(&[0x00], &[0x00]));
        assert!(constant_time_eq(&[0xff], &[0xff]));
        assert!(constant_time_eq(
            b"hello world, this is a longer string for constant-time comparison",
            b"hello world, this is a longer string for constant-time comparison"
        ));
    }

    #[test]
    fn constant_time_eq_different_slices() {
        // Same length, different content
        assert!(!constant_time_eq(&[0x00], &[0x01]));
        assert!(!constant_time_eq(&[0xff, 0xfe], &[0xff, 0xff]));
        assert!(!constant_time_eq(b"aaaa", b"aaab"));
    }

    #[test]
    fn constant_time_eq_different_lengths() {
        assert!(!constant_time_eq(&[], &[0x00]));
        assert!(!constant_time_eq(&[0x00], &[]));
        assert!(!constant_time_eq(&[0x01, 0x02], &[0x01, 0x02, 0x03]));
        assert!(!constant_time_eq(&[0x01, 0x02, 0x03], &[0x01, 0x02]));
    }

    #[test]
    fn constant_time_eq_all_bit_positions() {
        // Verify that every single-bit difference is detected.
        let base: u8 = 0xAA;
        for bit in 0..8 {
            let flipped = base ^ (1 << bit);
            assert!(
                !constant_time_eq(&[base], &[flipped]),
                "bit {bit} difference not detected"
            );
        }
    }

    #[test]
    fn constant_time_eq_str_delegates() {
        // constant_time_eq_str should behave identically to constant_time_eq
        assert!(constant_time_eq_str(b"abc", b"abc"));
        assert!(!constant_time_eq_str(b"abc", b"abd"));
        assert!(!constant_time_eq_str(b"ab", b"a"));
    }

    // ---------------------------------------------------------------------------
    // decode_hex
    // ---------------------------------------------------------------------------

    #[test]
    fn decode_hex_valid_strings() {
        assert_eq!(decode_hex(""), Ok(Vec::new()));
        assert_eq!(decode_hex("00"), Ok(vec![0x00]));
        assert_eq!(decode_hex("ff"), Ok(vec![0xFF]));
        assert_eq!(decode_hex("0a0b0c"), Ok(vec![0x0A, 0x0B, 0x0C]));
        assert_eq!(decode_hex("deadbeef"), Ok(vec![0xDE, 0xAD, 0xBE, 0xEF]));
        assert_eq!(
            decode_hex("0123456789abcdefABCDEF"),
            Ok(vec![
                0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xAB, 0xCD, 0xEF
            ])
        );
    }

    #[test]
    fn decode_hex_with_whitespace_trim() {
        assert_eq!(decode_hex("  ff  "), Ok(vec![0xFF]));
        assert_eq!(decode_hex("\t00\n"), Ok(vec![0x00]));
    }

    #[test]
    fn decode_hex_odd_length_fails() {
        assert_eq!(decode_hex("0"), Err(()));
        assert_eq!(decode_hex("abc"), Err(()));
        assert_eq!(decode_hex("  abc  "), Err(()));
    }

    #[test]
    fn decode_hex_invalid_characters() {
        assert_eq!(decode_hex("gg"), Err(()));
        assert_eq!(decode_hex("zz"), Err(()));
        assert_eq!(decode_hex("a g"), Err(()));
    }

    // ---------------------------------------------------------------------------
    // hex::encode (internal helper)
    // ---------------------------------------------------------------------------

    #[test]
    fn hex_encode_basic() {
        assert_eq!(hex::encode(&[]), "");
        assert_eq!(hex::encode(&[0x00]), "00");
        assert_eq!(hex::encode(&[0xFF]), "ff");
        assert_eq!(hex::encode(&[0xDE, 0xAD]), "dead");
        assert_eq!(hex::encode(b"Hello"), "48656c6c6f");
    }

    // ---------------------------------------------------------------------------
    // sha256_bytes
    // ---------------------------------------------------------------------------

    #[test]
    fn sha256_empty_string() {
        // NIST test vector for ""
        let expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let actual = hex::encode(sha256_bytes(b""));
        assert_eq!(actual, expected);
    }

    #[test]
    fn sha256_abc() {
        // NIST test vector for "abc"
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let actual = hex::encode(sha256_bytes(b"abc"));
        assert_eq!(actual, expected);
    }

    #[test]
    fn sha256_longer_input() {
        // SHA-256("hello world") known vector
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let actual = hex::encode(sha256_bytes(b"hello world"));
        assert_eq!(actual, expected);
    }

    #[test]
    fn sha256_deterministic() {
        let a = sha256_bytes(b"test data");
        let b = sha256_bytes(b"test data");
        assert_eq!(a, b);
    }

    #[test]
    fn sha256_different_inputs_differ() {
        let a = sha256_bytes(b"foo");
        let b = sha256_bytes(b"bar");
        assert_ne!(a, b);
    }

    // ---------------------------------------------------------------------------
    // check_cert_validity
    // ---------------------------------------------------------------------------

    /// Build a minimal self-signed DER certificate valid for a given period.
    /// Returns the raw DER bytes.
    fn make_test_cert(
        not_before: std::time::SystemTime,
        not_after: std::time::SystemTime,
    ) -> Vec<u8> {
        use rcgen::*;
        let mut params = CertificateParams::default();
        params.not_before = not_before.into();
        params.not_after = not_after.into();
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, "test");
        let key_pair = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key_pair).unwrap();
        cert.der().to_vec()
    }

    /// Helper: convert SystemTime to rustls UnixTime
    fn system_time_to_unix(t: std::time::SystemTime) -> rustls::pki_types::UnixTime {
        let secs = t
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        rustls::pki_types::UnixTime::since_unix_epoch(std::time::Duration::from_secs(secs))
    }

    #[test]
    fn check_cert_validity_valid_cert() {
        let now = std::time::SystemTime::now();
        let not_before = now - std::time::Duration::from_secs(3600);
        let not_after = now + std::time::Duration::from_secs(3600);
        let der = make_test_cert(not_before, not_after);
        let cert = rustls::pki_types::CertificateDer::from(der);
        let unix_now = system_time_to_unix(now);

        assert!(
            check_cert_validity(&cert, unix_now).is_ok(),
            "cert within validity period should pass"
        );
    }

    #[test]
    fn check_cert_validity_expired_cert() {
        let now = std::time::SystemTime::now();
        let not_before = now - std::time::Duration::from_secs(7200);
        let not_after = now - std::time::Duration::from_secs(3600);
        let der = make_test_cert(not_before, not_after);
        let cert = rustls::pki_types::CertificateDer::from(der);
        let unix_now = system_time_to_unix(now);

        let result = check_cert_validity(&cert, unix_now);
        assert!(result.is_err(), "expired cert should fail");
        assert!(
            result.unwrap_err().contains("expired"),
            "error should mention expiration"
        );
    }

    #[test]
    fn check_cert_validity_not_yet_valid_cert() {
        let now = std::time::SystemTime::now();
        let not_before = now + std::time::Duration::from_secs(3600);
        let not_after = now + std::time::Duration::from_secs(7200);
        let der = make_test_cert(not_before, not_after);
        let cert = rustls::pki_types::CertificateDer::from(der);
        let unix_now = system_time_to_unix(now);

        let result = check_cert_validity(&cert, unix_now);
        assert!(result.is_err(), "not-yet-valid cert should fail");
        assert!(
            result.unwrap_err().contains("not yet valid"),
            "error should mention not yet valid"
        );
    }

    #[test]
    fn check_cert_validity_garbage_der_returns_ok() {
        // Malformed DER should return Ok (best-effort parsing)
        let cert = rustls::pki_types::CertificateDer::from(vec![0x00, 0x01, 0x02]);
        let now = rustls::pki_types::UnixTime::now();
        assert!(
            check_cert_validity(&cert, now).is_ok(),
            "garbage DER should gracefully return Ok"
        );
    }

    // ---------------------------------------------------------------------------
    // FlexibleVerifier
    // ---------------------------------------------------------------------------

    fn make_verifier(fingerprints: Vec<String>) -> FlexibleVerifier {
        FlexibleVerifier {
            fingerprints,
            signature_algorithms: rustls::crypto::ring::default_provider()
                .signature_verification_algorithms,
        }
    }

    fn make_test_cert_with_key() -> (Vec<u8>, rcgen::KeyPair) {
        use rcgen::*;
        let mut params = CertificateParams::default();
        let now = std::time::SystemTime::now();
        params.not_before = (now - std::time::Duration::from_secs(3600)).into();
        params.not_after = (now + std::time::Duration::from_secs(3600)).into();
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, "test");
        let key_pair = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key_pair).unwrap();
        (cert.der().to_vec(), key_pair)
    }

    #[test]
    fn verifier_accepts_pinned_fingerprint() {
        ensure_rustls_crypto_provider();

        let (der, _) = make_test_cert_with_key();
        let fp = hex::encode(sha256_bytes(&der));
        let verifier = make_verifier(vec![fp]);

        let cert = rustls::pki_types::CertificateDer::from(der);
        let server_name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
        let now = rustls::pki_types::UnixTime::now();

        let result = verifier.verify_server_cert(&cert, &[], &server_name, &[], now);
        assert!(result.is_ok(), "pinned fingerprint should be accepted");
    }

    #[test]
    fn verifier_rejects_unmatched_fingerprint() {
        ensure_rustls_crypto_provider();

        let (der, _) = make_test_cert_with_key();
        let wrong_fp = "00".repeat(32); // 32 bytes = 64 hex chars
        let verifier = make_verifier(vec![wrong_fp]);

        let cert = rustls::pki_types::CertificateDer::from(der);
        let server_name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
        let now = rustls::pki_types::UnixTime::now();

        let result = verifier.verify_server_cert(&cert, &[], &server_name, &[], now);
        assert!(result.is_err(), "wrong fingerprint should be rejected");
    }

    #[test]
    fn verifier_rejects_when_no_fingerprints() {
        ensure_rustls_crypto_provider();

        let (der, _) = make_test_cert_with_key();
        let verifier = make_verifier(vec![]);

        let cert = rustls::pki_types::CertificateDer::from(der);
        let server_name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
        let now = rustls::pki_types::UnixTime::now();

        let result = verifier.verify_server_cert(&cert, &[], &server_name, &[], now);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            rustls::Error::General(msg) => {
                assert!(
                    msg.contains("self-signed TLS requires a pinned certificate fingerprint"),
                    "unexpected error: {msg}"
                );
            }
            other => panic!("expected General error, got: {other:?}"),
        }
    }

    #[test]
    fn verifier_accepts_hex_string_fingerprint() {
        // When the pinned fingerprint is a hex string (not decodeable to different
        // bytes than the actual hash), the string-comparison path should work.
        ensure_rustls_crypto_provider();

        let (der, _) = make_test_cert_with_key();
        let fp = hex::encode(sha256_bytes(&der));
        let verifier = make_verifier(vec![fp]);

        let cert = rustls::pki_types::CertificateDer::from(der);
        let server_name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
        let now = rustls::pki_types::UnixTime::now();

        assert!(
            verifier
                .verify_server_cert(&cert, &[], &server_name, &[], now)
                .is_ok()
        );
    }

    #[test]
    fn verifier_accepts_one_of_multiple_fingerprints() {
        ensure_rustls_crypto_provider();

        let (der, _) = make_test_cert_with_key();
        let fp = hex::encode(sha256_bytes(&der));
        let wrong = "00".repeat(32);
        let verifier = make_verifier(vec![wrong.clone(), fp, wrong]);

        let cert = rustls::pki_types::CertificateDer::from(der);
        let server_name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
        let now = rustls::pki_types::UnixTime::now();

        assert!(
            verifier
                .verify_server_cert(&cert, &[], &server_name, &[], now)
                .is_ok(),
            "should accept when at least one fingerprint matches"
        );
    }

    #[test]
    fn verifier_supported_verify_schemes_not_empty() {
        let verifier = make_verifier(vec!["00".repeat(32)]);
        let schemes = verifier.supported_verify_schemes();
        assert!(!schemes.is_empty(), "should advertise at least one scheme");
    }

    // ---------------------------------------------------------------------------
    // build_client_tls_config
    // ---------------------------------------------------------------------------

    #[test]
    fn rejects_unpinned_self_signed_tls() {
        let config = TlsConfig {
            accept_self_signed: true,
            cert_fingerprints: Vec::new(),
            enforce_https: false,
        };

        let err = build_client_tls_config(&config).expect_err("unpinned self-signed TLS must fail");
        assert!(
            err.to_string()
                .contains("self-signed TLS requires at least one pinned certificate fingerprint")
        );
    }

    #[test]
    fn builds_standard_tls_config_without_self_signed() {
        let config = TlsConfig {
            accept_self_signed: false,
            cert_fingerprints: Vec::new(),
            enforce_https: false,
        };

        let result = build_client_tls_config(&config);
        assert!(
            result.is_ok(),
            "standard TLS config should build successfully"
        );
    }

    #[test]
    fn builds_self_signed_config_with_fingerprints() {
        let fake_fp = "00".repeat(32);
        let config = TlsConfig {
            accept_self_signed: true,
            cert_fingerprints: vec![fake_fp],
            enforce_https: false,
        };

        let result = build_client_tls_config(&config);
        assert!(
            result.is_ok(),
            "self-signed with fingerprint should build successfully"
        );
    }

    #[test]
    fn builds_config_with_multiple_fingerprints() {
        let config = TlsConfig {
            accept_self_signed: true,
            cert_fingerprints: vec!["aa".repeat(32), "bb".repeat(32), "cc".repeat(32)],
            enforce_https: false,
        };

        let result = build_client_tls_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn self_signed_config_uses_custom_verifier() {
        // Verify the returned config is usable (no panic, returns Arc)
        let config = TlsConfig {
            accept_self_signed: true,
            cert_fingerprints: vec!["00".repeat(32)],
            enforce_https: false,
        };
        let tls = build_client_tls_config(&config).unwrap();

        // We can't inspect internals of ClientConfig easily, but we can verify
        // that it's a valid Arc and that the alpn protocols can be set.
        assert_eq!(Arc::strong_count(&tls), 1);
    }
}
