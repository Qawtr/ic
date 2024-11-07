use crate::*;
use ic_types::Randomness;
use serde::{
    Deserialize,
    Serialize,
};

/*
Implements BIP340 Schnorr signatures over the secp256k1 curve

https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki
 */

fn fix_to_even_y(pt: &EccPoint) -> CanisterThresholdResult<(EccPoint, bool)> {
    if pt.is_y_even()? {
        Ok((pt.clone(), false))
    } else {
        Ok((pt.negate(), true))
    }
}

/// Compute the Fiat-Shamir challenge as described in BIP340
///
/// Schnorr signatures are effectively a Sigma protocol proving
/// knowledge of discrete logarithm, made non-interactive using
/// the Fiat-Shamir heuristic; the interactive random challenge
/// is replaced by a random oracle applied to the transcript
/// so far.
///
/// See <https://www.zkdocs.com/docs/zkdocs/zero-knowledge-protocols/schnorr/>
/// and <https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki#default-signing>
fn bip340_challenge_hash(
    r: &EccPoint,
    p: &EccPoint,
    msg: &[u8],
) -> CanisterThresholdResult<EccScalar> {
    let tag = "BIP0340/challenge";

    let h_tag = ic_crypto_sha2::Sha256::hash(tag.as_bytes());

    let mut sha256 = ic_crypto_sha2::Sha256::new();
    sha256.write(&h_tag);
    sha256.write(&h_tag);
    sha256.write(&r.serialize_bip340()?);
    sha256.write(&p.serialize_bip340()?);
    sha256.write(msg);
    let e = sha256.finish();

    EccScalar::from_bytes_wide(EccCurveType::K256, &e)
}

/// Taproot key derivation
///
/// See BIP-341 <https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#user-content-Constructing_and_spending_Taproot_outputs> for details.
fn compute_taproot_tweak(pk: &EccPoint, h: &[u8]) -> CanisterThresholdResult<EccScalar> {
    let tag = "TapTweak";

    let h_tag = ic_crypto_sha2::Sha256::hash(tag.as_bytes());

    let mut sha256 = ic_crypto_sha2::Sha256::new();
    sha256.write(&h_tag);
    sha256.write(&h_tag);
    sha256.write(&pk.affine_x_bytes()?);
    sha256.write(h);

    // This fails if the hash is greater than the group order;
    // this happens with small probability but the failure is
    // mandated by BIP-0341
    EccScalar::deserialize(EccCurveType::K256, &sha256.finish())
        .map_err(|_| CanisterThresholdError::InvalidScalar)
}

/// Presignature rerandomization
///
/// Malicious nodes can cause biases in the presignature R transcript
/// due to the use of unblinded commitments in the RandomUnmasked case.
/// We prevent this from being an issue by rerandomizing the R value
/// using information that is not available until the point the signature
///
/// This does not match what BIP340 does; they instead define a deterministic
/// scheme for choosing the presignature. This is not practical for us to use,
/// since their approach requires knowledge of the entire private key, which is
/// impractical without a generic MPC setup which is computationally expensive.
///
/// The rerandomization process includes also the step for deriving the subkey
/// that is used for this particular caller (based on derivation path, which
/// includes the canister id). This is because we use the derived key as one
/// of the inputs to the presignature rerandomization step.
///
/// For more information about rerandomization of Schnorr presignatures see
/// "The many faces of Schnorr", Victor Shoup <https://eprint.iacr.org/2023/1019>
struct RerandomizedPresignature {
    /// The derived public key
    derived_key: EccPoint,
    /// The discrete log of the difference between the derived public key
    /// and the master public key
    key_tweak: EccScalar,
    /// The rerandomized presignature commitment
    randomized_pre_sig: EccPoint,
    /// The discrete log of the difference between the rerandomized presignature
    /// and the presignature transcript generated by the IDKG
    presig_randomizer: EccScalar,
    /// Do we need to flip the key share?
    /// This is needed ~50% of the time to handle BIP340's even-y convention
    flip_key: bool,
}

impl RerandomizedPresignature {
    fn compute(
        message: &[u8],
        randomness: &Randomness,
        derivation_path: &DerivationPath,
        taproot_tree_root: Option<&[u8]>,
        key_transcript: &IDkgTranscriptInternal,
        presig_transcript: &IDkgTranscriptInternal,
    ) -> CanisterThresholdResult<Self> {
        let pre_sig = match &presig_transcript.combined_commitment {
            // random unmasked case
            // unlike for ECDSA we require the Schnorr R be generated by random unmasked only
            CombinedCommitment::BySummation(PolynomialCommitment::Simple(c)) => c.constant_term(),
            _ => return Err(CanisterThresholdError::UnexpectedCommitmentType),
        };

        let curve = pre_sig.curve_type();

        // BIP340 Schnorr is only defined for secp256k1
        if curve != EccCurveType::K256 {
            return Err(CanisterThresholdError::UnexpectedCommitmentType);
        }

        let idkg_key = key_transcript.constant_term();

        let (key_tweak, _chain_key) = derivation_path.derive_tweak(&idkg_key)?;

        // Rerandomize presignature
        let mut ro = RandomOracle::new(DomainSep::RerandomizePresig(IdkgProtocolAlgorithm::Bip340));

        ro.add_bytestring("randomness", &randomness.get())?;
        ro.add_bytestring("message", message)?;
        ro.add_point("pre_sig", &pre_sig)?;
        ro.add_point("key_transcript", &idkg_key)?;
        ro.add_scalar("key_tweak", &key_tweak)?;

        if let Some(ttr) = taproot_tree_root {
            ro.add_bytestring("taproot_tree_root", ttr)?;
        }

        let presig_randomizer = ro.output_scalar(curve)?;

        let randomized_pre_sig =
            pre_sig.add_points(&EccPoint::generator_g(curve).scalar_mul(&presig_randomizer)?)?;
        let derived_key =
            idkg_key.add_points(&EccPoint::generator_g(curve).scalar_mul(&key_tweak)?)?;

        // Now some adjustments to handle BIP340's even-y convention
        let (derived_key, flip_key, key_tweak) = if let Some(ttr) = taproot_tree_root {
            // If taproot we have to perform yet another additive tweak
            let tap_tweak = compute_taproot_tweak(&derived_key, ttr)?;
            let rr_even = derived_key.is_y_even()?;
            let g_tweak = EccPoint::mul_by_g(&tap_tweak);

            if rr_even {
                let (tweak_key, flip_key) = fix_to_even_y(&derived_key.add_points(&g_tweak)?)?;
                let tweak_sum = key_tweak.add(&tap_tweak)?;
                (tweak_key, flip_key, tweak_sum)
            } else {
                let (tweak_key, flip_key) = fix_to_even_y(&derived_key.sub_points(&g_tweak)?)?;
                let tweak_sum = key_tweak.sub(&tap_tweak)?;
                (tweak_key, flip_key, tweak_sum)
            }
        } else {
            let (dk, flip_key) = fix_to_even_y(&derived_key)?;
            (dk, flip_key, key_tweak)
        };

        Ok(Self {
            derived_key,
            key_tweak,
            randomized_pre_sig,
            presig_randomizer,
            flip_key,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ThresholdBip340SignatureShareInternal {
    s: EccScalar,
}

impl ThresholdBip340SignatureShareInternal {
    pub(crate) fn new(
        derivation_path: &DerivationPath,
        message: &[u8],
        taproot_tree_root: Option<&[u8]>,
        randomness: Randomness,
        key_transcript: &IDkgTranscriptInternal,
        key_opening: &CommitmentOpening,
        presig_transcript: &IDkgTranscriptInternal,
        presig_opening: &CommitmentOpening,
    ) -> CanisterThresholdResult<Self> {
        let rerandomized = RerandomizedPresignature::compute(
            message,
            &randomness,
            derivation_path,
            taproot_tree_root,
            key_transcript,
            presig_transcript,
        )?;

        let (presig_r, flip_presig_share) = fix_to_even_y(&rerandomized.randomized_pre_sig)?;

        let key_opening = match key_opening {
            CommitmentOpening::Simple(s) => s,
            _ => return Err(CanisterThresholdError::UnexpectedCommitmentType),
        };

        let presig_opening = match presig_opening {
            CommitmentOpening::Simple(s) => s,
            _ => return Err(CanisterThresholdError::UnexpectedCommitmentType),
        };

        let e = bip340_challenge_hash(&presig_r, &rerandomized.derived_key, message)?;

        let tweaked_x = key_opening.add(&rerandomized.key_tweak)?;

        /*
         * The linear combination used to create the share varies based on if we
         * had to negate pk and/or r in order to use the "correct" even-y point.
         */

        let xh = if rerandomized.flip_key {
            tweaked_x.negate().mul(&e)?
        } else {
            tweaked_x.mul(&e)?
        };

        let r_plus_randomizer = presig_opening.add(&rerandomized.presig_randomizer)?;

        let share = if flip_presig_share {
            xh.sub(&r_plus_randomizer)?
        } else {
            xh.add(&r_plus_randomizer)?
        };

        Ok(Self { s: share })
    }

    /// Verify a Schnorr signature share
    ///
    /// Schnorr signature shares are quite simple in that they are (ignoring
    /// rerandomization and even-y issues) simply [s] = [k]*e + [r]
    /// where [k] is the key share, [r] is the share of the presignature, and e
    /// is the challenge (which is known to all parties).
    ///
    /// The important thing to note here is that this expression itself gives a
    /// Schnorr signature, namely a signature of e with respect to the node's
    /// share of the key and presignature.  Since the public commitments to
    /// these shares are unblinded, it is possible for us to compute the public
    /// key and presignature associated with the node's shares by evaluating the
    /// respective commmitments at the signer's index
    pub(crate) fn verify(
        &self,
        derivation_path: &DerivationPath,
        message: &[u8],
        taproot_tree_root: Option<&[u8]>,
        randomness: Randomness,
        signer_index: NodeIndex,
        key_transcript: &IDkgTranscriptInternal,
        presig_transcript: &IDkgTranscriptInternal,
    ) -> CanisterThresholdResult<()> {
        let rerandomized = RerandomizedPresignature::compute(
            message,
            &randomness,
            derivation_path,
            taproot_tree_root,
            key_transcript,
            presig_transcript,
        )?;

        let (presig_r, flip_r) = fix_to_even_y(&rerandomized.randomized_pre_sig)?;

        let e = bip340_challenge_hash(&presig_r, &rerandomized.derived_key, message)?;

        let node_pk = key_transcript
            .combined_commitment
            .commitment()
            .evaluate_at(signer_index)?
            .add_points(&EccPoint::mul_by_g(&rerandomized.key_tweak))?;
        let node_r = presig_transcript
            .combined_commitment
            .commitment()
            .evaluate_at(signer_index)?
            .add_points(&EccPoint::mul_by_g(&rerandomized.presig_randomizer))?;

        // Have to account for negating pk and/or R:
        let node_pk = if rerandomized.flip_key {
            node_pk.negate()
        } else {
            node_pk
        };
        let node_r = if flip_r { node_r.negate() } else { node_r };

        let lhs = EccPoint::mul_by_g(&self.s);
        let hp = node_pk.scalar_mul(&e)?;
        let rhs = node_r.add_points(&hp)?;

        if rhs == lhs {
            Ok(())
        } else {
            Err(CanisterThresholdError::InvalidSignatureShare)
        }
    }

    pub fn serialize(
        &self,
    ) -> Result<Vec<u8>, ThresholdBip340SignatureShareInternalSerializationError> {
        serde_cbor::to_vec(self)
            .map_err(|e| ThresholdBip340SignatureShareInternalSerializationError(e.to_string()))
    }

    pub fn deserialize(
        raw: &[u8],
    ) -> Result<Self, ThresholdBip340SignatureShareInternalSerializationError> {
        serde_cbor::from_slice::<Self>(raw)
            .map_err(|e| ThresholdBip340SignatureShareInternalSerializationError(e.to_string()))
    }
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ThresholdBip340SignatureShareInternalSerializationError(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ThresholdBip340CombinedSignatureInternal {
    r: EccPoint,
    s: EccScalar,
}

impl ThresholdBip340CombinedSignatureInternal {
    /// Serialize in the format BIP340 expects, x coordinate only
    pub fn serialize(
        &self,
    ) -> Result<Vec<u8>, ThresholdBip340SignatureShareInternalSerializationError> {
        let mut v = vec![];
        let serialized_p = self.r.serialize_bip340().map_err(|e| {
            ThresholdBip340SignatureShareInternalSerializationError(format!(
                "Failed to serialize r: {:?}",
                e
            ))
        })?;
        v.extend_from_slice(&serialized_p);
        v.extend_from_slice(&self.s.serialize());
        Ok(v)
    }

    /// Deserialize in the format BIP340 expects, x coordinate only
    pub fn deserialize(
        bytes: &[u8],
    ) -> Result<Self, ThresholdBip340SignatureShareInternalSerializationError> {
        const K256: EccCurveType = EccCurveType::K256;
        const POINT_LEN: usize = match K256.point_bytes_bip340() {
            Some(point_len) => point_len,
            None => panic!("const panic!: failed to determine BIP340 point size at compile time"),
        };
        const EXPECTED_LEN: usize = K256.scalar_bytes() + POINT_LEN;

        if bytes.len() != EXPECTED_LEN {
            return Err(ThresholdBip340SignatureShareInternalSerializationError(
                format!(
                    "Bad signature length, expected {EXPECTED_LEN} but got {}",
                    bytes.len()
                ),
            ));
        }

        let (point_bytes, scalar_bytes) = bytes.split_at(POINT_LEN);

        let r = EccPoint::deserialize_bip340(K256, point_bytes).map_err(|e| {
            ThresholdBip340SignatureShareInternalSerializationError(format!("Invalid r: {:?}", e))
        })?;

        let s = EccScalar::deserialize(K256, scalar_bytes).map_err(|e| {
            ThresholdBip340SignatureShareInternalSerializationError(format!("Invalid s: {:?}", e))
        })?;

        Ok(Self { r, s })
    }

    /// Combine shares into a BIP340 Schnorr signature
    pub fn new(
        derivation_path: &DerivationPath,
        message: &[u8],
        taproot_tree_root: Option<&[u8]>,
        randomness: Randomness,
        key_transcript: &IDkgTranscriptInternal,
        presig_transcript: &IDkgTranscriptInternal,
        reconstruction_threshold: NumberOfNodes,
        sig_shares: &BTreeMap<NodeIndex, ThresholdBip340SignatureShareInternal>,
    ) -> CanisterThresholdResult<Self> {
        let reconstruction_threshold = reconstruction_threshold.get() as usize;
        if sig_shares.len() < reconstruction_threshold {
            return Err(CanisterThresholdError::InsufficientDealings);
        }

        let rerandomized = RerandomizedPresignature::compute(
            message,
            &randomness,
            derivation_path,
            taproot_tree_root,
            key_transcript,
            presig_transcript,
        )?;

        let (presig_r, _) = fix_to_even_y(&rerandomized.randomized_pre_sig)?;

        let mut x_values = Vec::with_capacity(reconstruction_threshold);
        let mut samples = Vec::with_capacity(reconstruction_threshold);

        for (index, sig_share) in sig_shares.iter().take(reconstruction_threshold) {
            x_values.push(*index);
            samples.push(sig_share.s.clone());
        }

        let coefficients = LagrangeCoefficients::at_zero(EccCurveType::K256, &x_values)?;
        let combined_s = coefficients.interpolate_scalar(&samples)?;

        Ok(Self {
            r: presig_r,
            s: combined_s,
        })
    }

    /// Verify a BIP340 Schnorr signature
    ///
    /// In addition to normal signature verification, this also checks
    /// that the signature was generated using a specific presignature
    /// transcript
    pub fn verify(
        &self,
        derivation_path: &DerivationPath,
        message: &[u8],
        taproot_tree_root: Option<&[u8]>,
        randomness: Randomness,
        presig_transcript: &IDkgTranscriptInternal,
        key_transcript: &IDkgTranscriptInternal,
    ) -> CanisterThresholdResult<()> {
        if self.r.is_infinity()? || !self.r.is_y_even()? || self.s.is_zero() {
            return Err(CanisterThresholdError::InvalidSignature);
        }

        let rerandomized = RerandomizedPresignature::compute(
            message,
            &randomness,
            derivation_path,
            taproot_tree_root,
            key_transcript,
            presig_transcript,
        )?;

        let (presig_r, _) = fix_to_even_y(&rerandomized.randomized_pre_sig)?;

        if self.r != presig_r {
            return Err(CanisterThresholdError::InvalidSignature);
        }

        let e = bip340_challenge_hash(&presig_r, &rerandomized.derived_key, message)?;

        // R = s*G - e*P
        let g = EccPoint::generator_g(EccCurveType::K256);
        let rp = EccPoint::mul_2_points(&g, &self.s, &rerandomized.derived_key, &e.negate())?;

        // We already checked above that self.r is not infinity and has even y:
        if rp != self.r {
            return Err(CanisterThresholdError::InvalidSignature);
        }

        // accept:
        Ok(())
    }
}
