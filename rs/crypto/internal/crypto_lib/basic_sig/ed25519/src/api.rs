//! API for Ed25519 basic signature
use super::types;
use ic_crypto_internal_basic_sig_der_utils as der_utils;
use ic_crypto_internal_seed::Seed;
use ic_crypto_secrets_containers::{SecretArray, SecretBytes};
use ic_types::crypto::{AlgorithmId, CryptoError, CryptoResult};
use rand::{CryptoRng, Rng};
use std::convert::TryFrom;
use zeroize::Zeroize;

#[cfg(test)]
mod tests;

/// Generates an Ed25519 keypair.
pub fn keypair_from_rng<R: Rng + CryptoRng>(
    csprng: &mut R,
) -> (types::SecretKeyBytes, types::PublicKeyBytes) {
    let mut signing_key = ed25519_consensus::SigningKey::new(csprng);
    let sk = types::SecretKeyBytes(SecretArray::new_and_dont_zeroize_argument(
        signing_key.as_bytes(),
    ));
    let pk = types::PublicKeyBytes(signing_key.verification_key().to_bytes());
    signing_key.zeroize();
    (sk, pk)
}

/// The object identifier for Ed25519 public keys
///
/// See [RFC 8410](https://tools.ietf.org/html/rfc8410).
pub fn algorithm_identifier() -> der_utils::PkixAlgorithmIdentifier {
    der_utils::PkixAlgorithmIdentifier::new_with_empty_param(simple_asn1::oid!(1, 3, 101, 112))
}

/// Decodes an Ed25519 public key from a DER-encoding according to
/// [RFC 8410, Section 4](https://tools.ietf.org/html/rfc8410#section-4).
///
/// Uses the Ed25519 object identifier (OID) 1.3.101.112 (see [RFC 8410](https://tools.ietf.org/html/rfc8410)).
///
/// # Errors
/// * `MalformedPublicKey` if the input is not a valid DER-encoding according to
///   RFC 8410, or the OID in incorrect, or the key length is incorrect.
pub fn public_key_from_der(pk_der: &[u8]) -> CryptoResult<types::PublicKeyBytes> {
    let expected_pk_len = 32;
    let pk_bytes = der_utils::parse_public_key(
        pk_der,
        AlgorithmId::Ed25519,
        algorithm_identifier(),
        Some(expected_pk_len),
    )?;
    types::PublicKeyBytes::try_from(pk_bytes)
}

/// Encodes the given `key` as DER-encoded Ed25519 public key according to
/// [RFC 8410, Section 4](https://tools.ietf.org/html/rfc8410#section-4).
///
/// Uses the Ed25519 object identifier (OID) 1.3.101.112 (see [RFC 8410](https://tools.ietf.org/html/rfc8410)).
pub fn public_key_to_der(key: types::PublicKeyBytes) -> Vec<u8> {
    // Prefixing the following bytes to the key is sufficient to DER-encode it.
    let mut der_pk = vec![
        48, 42, // A sequence of 42 bytes follows.
        48, 5, // An element of 5 bytes follows.
        6, 3, 43, 101, 112, // The OID
        3, 33, // A bitstring of 33 bytes follows.
        0,  // The bitstring (32 bytes) is divisible by 8
    ];
    der_pk.extend_from_slice(&key.0);
    der_pk
}

/// An error indicating that decoding of a key failed
#[derive(Clone, Debug)]
pub enum KeyDecodingError {
    InvalidEncoding(String),
    InternalError(String),
}

/// Deserializes an Ed25519 secret key from DER-encoding according to PKCS#8 v1 (RFC 5208).
/// Uses the Ed25519 object identifier (OID) 1.3.101.112 (see [RFC 8410](https://tools.ietf.org/html/rfc8410)).
/// Returns None if the key is not encoded according to PKCS#8.
pub fn secret_key_from_pkcs8_v1_der(
    der: &SecretBytes,
) -> Result<types::SecretKeyBytes, KeyDecodingError> {
    let pkcs8_v1_ed25519_prefix: [u8; 16] =
        [48, 46, 2, 1, 0, 48, 5, 6, 3, 43, 101, 112, 4, 34, 4, 32];
    let sk_der = der.expose_secret();
    if sk_der.len() != 48 || !sk_der.starts_with(&pkcs8_v1_ed25519_prefix) {
        return Err(KeyDecodingError::InvalidEncoding(
            "invalid PKCS#8 v1 DER-encoding of Ed25519 secret key".to_string(),
        ));
    };
    let sk: &[u8; 32] = sk_der[16..48].try_into().map_err(|_| {
        KeyDecodingError::InternalError(
            "input is not 48 bytes and/or prefix is not 16 bytes".to_string(),
        )
    })?;
    Ok(types::SecretKeyBytes(
        SecretArray::new_and_dont_zeroize_argument(sk),
    ))
}

/// Serializes an Ed25519 private key to PKCS8 (v1) format in DER encoding (RFC 5208).
/// The serialization does not include the public key.
pub fn secret_key_to_pkcs8_v1_der(sk: &types::SecretKeyBytes) -> SecretBytes {
    let mut der = vec![
        48, 46, // A sequence of 46 bytes follows.
        2, 1, // An integer denoting version
        0, // 0 if secret key only, 1 if public key is also present
        48, 5, // An element of 5 bytes follows
        6, 3, 43, 101, 112, // The OID
        4, 34, // An octet string of 34 bytes follows.
        4, 32, // An octet string of 32 bytes follows.
    ];
    der.extend_from_slice(sk.0.expose_secret());
    SecretBytes::new(der)
}

/// Serializes an Ed25519 key pair to PKCS8 v2 format in DER encoding (RFC 5958).
/// The serialization includes both the secret and the public key.
pub fn secret_key_to_pkcs8_v2_der(
    sk: &types::SecretKeyBytes,
    pk: &types::PublicKeyBytes,
) -> SecretBytes {
    // Prefixing the following bytes to the secret key.
    let mut der_sk = vec![
        48, 83, // A sequence of 83 bytes follows.
        2, 1, // An integer denoting version
        1, // 0 if secret key only, 1 if public key is also present
        48, 5, // An element of 5 bytes follows
        6, 3, 43, 101, 112, // The OID
        4, 34, // An octet string of 34 bytes follows.
        4, 32, // An octet string of 32 bytes follows.
    ];
    der_sk.extend_from_slice(sk.0.expose_secret());

    // Prefixing the following bytes to the public key.
    der_sk.extend_from_slice(&[
        161, 35, // An explicitly tagged with 35 bytes.
        3, 33, // A bitstring of 33 bytes follows.
        0,  // The bitstring (32 bytes) is divisible by 8
    ]);
    der_sk.extend_from_slice(&pk.0);
    SecretBytes::new(der_sk)
}

/// Signs a message with an Ed25519 secret key.
///
/// # Errors
/// * `MalformedSecretKey` if the secret key is malformed
pub fn sign(msg: &[u8], sk: &types::SecretKeyBytes) -> CryptoResult<types::SignatureBytes> {
    let mut signing_key = ed25519_consensus::SigningKey::from(*sk.0.expose_secret());
    let signature = signing_key.sign(msg);
    signing_key.zeroize();
    Ok(types::SignatureBytes(signature.to_bytes()))
}

/// Verifies a signature using an Ed25519 public key.
///
/// # Errors
/// * `MalformedPublicKey` if the public key is malformed
/// * `SignatureVerification` if the signature is invalid
/// * `MalformedSignature` if the signature is malformed
pub fn verify(
    sig: &types::SignatureBytes,
    msg: &[u8],
    pk: &types::PublicKeyBytes,
) -> CryptoResult<()> {
    let entry = crate::cache::SignatureCacheEntry::new(&pk.0, &sig.0, msg);

    if crate::cache::Cache::<crate::cache::SignatureCacheEntry>::global().contains(&entry) {
        return Ok(());
    }

    let verification_key = ed25519_consensus::VerificationKey::try_from(pk.0).map_err(|e| {
        CryptoError::MalformedPublicKey {
            algorithm: AlgorithmId::Ed25519,
            key_bytes: Some(pk.0.to_vec()),
            internal_error: e.to_string(),
        }
    })?;
    let sig = ed25519_consensus::Signature::from(sig.0);

    let result =
        verification_key
            .verify(&sig, msg)
            .map_err(|e| CryptoError::SignatureVerification {
                algorithm: AlgorithmId::Ed25519,
                public_key_bytes: verification_key.to_bytes().to_vec(),
                sig_bytes: sig.to_bytes().to_vec(),
                internal_error: e.to_string(),
            });

    if result.is_ok() {
        crate::cache::Cache::<crate::cache::SignatureCacheEntry>::global().insert(&entry);
    };

    result
}

/// Verifies one or more signatures of the same message using
/// the respective Ed25519 public key(s).
///
/// # Errors
/// * `MalformedPublicKey` if the public key is malformed
/// * `SignatureVerification` if the signature is invalid
/// * `MalformedSignature` if the signature is malformed
pub fn verify_batch(
    key_signature_map: &[(&types::PublicKeyBytes, &types::SignatureBytes)],
    msg: &[u8],
    seed: Seed,
) -> CryptoResult<()> {
    let uncached: Vec<_> = key_signature_map
        .iter()
        .map(|&(pk, sig)| {
            (
                (pk, sig),
                crate::cache::SignatureCacheEntry::new(&pk.0, &sig.0, msg),
            )
        })
        .filter(|((_pk, _sig), entry)| {
            !crate::cache::Cache::<crate::cache::SignatureCacheEntry>::global().contains(entry)
        })
        .collect();

    if uncached.is_empty() {
        return Ok(());
    }

    let uncached_key_signature_map_vec: Vec<_> = uncached.iter().map(|(x, _)| *x).collect();
    let key_signature_map = uncached_key_signature_map_vec.as_slice();

    let mut batch_verifier = ed25519_consensus::batch::Verifier::new();
    for (pk, sig) in key_signature_map {
        let verification_key = ed25519_consensus::VerificationKey::try_from(pk.0).map_err(|e| {
            CryptoError::MalformedPublicKey {
                algorithm: AlgorithmId::Ed25519,
                key_bytes: Some(pk.0.to_vec()),
                internal_error: e.to_string(),
            }
        })?;
        let verification_key_bytes: ed25519_consensus::VerificationKeyBytes =
            verification_key.into();
        let sig = ed25519_consensus::Signature::from(sig.0);
        batch_verifier.queue((verification_key_bytes, sig, &msg));
    }

    let rng = seed.into_rng();
    let result = batch_verifier
        .verify(rng)
        .map_err(|e| CryptoError::SignatureVerification {
            algorithm: AlgorithmId::Ed25519,
            public_key_bytes: vec![],
            sig_bytes: vec![],
            internal_error: e.to_string(),
        });

    if result.is_ok() {
        for (_, entry) in uncached {
            crate::cache::Cache::<crate::cache::SignatureCacheEntry>::global().insert(&entry);
        }
    }

    result
}

/// Verifies whether the given key is a valid Ed25519 public key.
///
/// This includes checking that the key is a point on the curve and
/// in the right subgroup.
pub fn verify_public_key(pk: &types::PublicKeyBytes) -> bool {
    let entry = crate::cache::PkCheckCacheEntry::new(&pk.0);
    if crate::cache::Cache::<crate::cache::PkCheckCacheEntry>::global().contains(&entry) {
        return true;
    }

    let result = match curve25519_dalek::edwards::CompressedEdwardsY(pk.0).decompress() {
        None => false,
        Some(edwards_point) => edwards_point.is_torsion_free(),
    };

    if result {
        crate::cache::Cache::<crate::cache::PkCheckCacheEntry>::global().insert(&entry);
    }

    result
}
