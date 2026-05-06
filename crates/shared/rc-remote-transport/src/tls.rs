//! TLS configuration helpers for secure connections.

use std::sync::Arc;

use crate::TlsConfig;

/// Build a rustls client config from our TlsConfig.
pub fn build_client_tls_config(config: &TlsConfig) -> anyhow::Result<Arc<rustls::ClientConfig>> {
    let mut root_store = rustls::RootCertStore::empty();
    let result = rustls_native_certs::load_native_certs();
    for cert in result.certs {
        root_store.add(cert)?;
    }
    if !result.errors.is_empty() {
        tracing::warn!("native cert loading errors: {:?}", result.errors);
    }

    if config.accept_self_signed {
        let builder = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(FlexibleVerifier {
                fingerprints: config.cert_fingerprints.clone(),
            }))
            .with_no_client_auth();
        return Ok(Arc::new(builder));
    }

    let builder = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(Arc::new(builder))
}

/// Certificate verifier that can accept self-signed certs with optional fingerprint pinning.
#[derive(Debug)]
struct FlexibleVerifier {
    fingerprints: Vec<String>,
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
        // If fingerprints are pinned, verify the leaf cert matches.
        if !self.fingerprints.is_empty() {
            let fp = sha256_hex(end_entity.as_ref());
            if self.fingerprints.iter().any(|p| p.eq_ignore_ascii_case(&fp)) {
                return Ok(rustls::client::danger::ServerCertVerified::assertion());
            }
            return Err(rustls::Error::General(
                "certificate fingerprint does not match any pinned value".into(),
            ));
        }
        // No pins — accept any cert (LAN self-signed mode, user opted in).
        let _ = (intermediates, server_name, ocsp_response, now);
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        use rustls::SignatureScheme::*;
        vec![
            ECDSA_NISTP256_SHA256,
            ECDSA_NISTP384_SHA384,
            RSA_PKCS1_SHA256,
            RSA_PKCS1_SHA384,
            RSA_PKCS1_SHA512,
            ED25519,
        ]
    }
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(data);
    hex::encode(digest)
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}