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

#[allow(dead_code)]
fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha256_bytes(data))
}

/// Compute the raw SHA-256 digest bytes.
fn sha256_bytes(data: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).to_vec()
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
    if s.len() % 2 != 0 {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
