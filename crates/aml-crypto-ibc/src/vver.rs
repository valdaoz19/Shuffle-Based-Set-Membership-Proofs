//! Protocol 2: Π.VVer — prove commitment opens to a known value.

use curve25519_dalek::{RistrettoPoint, Scalar};
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::commitment::{IbcCommitment, IbcOpening};
use crate::params::PublicParams;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VVerProof {
    pub t: RistrettoPoint,
    pub z_iden: Scalar,
    pub z_ran: Scalar,
    pub challenge: Scalar,
}

pub struct VVerProver;

impl VVerProver {
    pub fn prove<R: RngCore + CryptoRng>(
        pp: &PublicParams,
        opening: &IbcOpening,
        commitment: &IbcCommitment,
        claimed_value: u64,
        rng: &mut R,
    ) -> VVerProof {
        assert_eq!(
            opening.value, claimed_value,
            "VVer requires prover to know matching value"
        );

        let alpha_iden = Scalar::random(rng);
        let alpha_ran = Scalar::random(rng);
        let t = pp.g_iden * alpha_iden + pp.g_ran * alpha_ran;

        let beta = Self::derive_challenge(pp, commitment, claimed_value, &t);

        let z_iden = beta * Scalar::from(opening.id) + alpha_iden;
        let z_ran = beta * opening.randomness + alpha_ran;

        VVerProof {
            t,
            z_iden,
            z_ran,
            challenge: beta,
        }
    }

    fn derive_challenge(
        pp: &PublicParams,
        commitment: &IbcCommitment,
        claimed_value: u64,
        t: &RistrettoPoint,
    ) -> Scalar {
        use sha3::{Digest, Sha3_512};
        let mut hasher = Sha3_512::new();
        hasher.update(b"aml-ibc-vver-v1");
        hasher.update(pp.g_iden.compress().as_bytes());
        hasher.update(pp.g_val.compress().as_bytes());
        hasher.update(pp.g_ran.compress().as_bytes());
        hasher.update(commitment.0.compress().as_bytes());
        hasher.update(claimed_value.to_le_bytes());
        hasher.update(t.compress().as_bytes());
        Scalar::from_hash(hasher)
    }
}

pub struct VVerVerifier;

impl VVerVerifier {
    pub fn verify(
        pp: &PublicParams,
        commitment: &IbcCommitment,
        claimed_value: u64,
        proof: &VVerProof,
    ) -> bool {
        let expected_beta = VVerProver::derive_challenge(pp, commitment, claimed_value, &proof.t);
        if proof.challenge != expected_beta {
            return false;
        }

        let lhs = pp.g_iden * proof.z_iden + pp.g_ran * proof.z_ran;
        let val_term = pp.g_val * Scalar::from(claimed_value);
        let adjusted = commitment.0 - val_term;
        let rhs = proof.t + adjusted * proof.challenge;
        lhs == rhs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commitment::IbcScheme;
    use rand::rngs::OsRng;

    #[test]
    fn vver_accepts_correct_value() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let (c, o) = IbcScheme::commit(&pp, 42, 50_000, &mut rng);
        let proof = VVerProver::prove(&pp, &o, &c, 50_000, &mut rng);
        assert!(VVerVerifier::verify(&pp, &c, 50_000, &proof));
    }

    #[test]
    fn vver_rejects_wrong_value() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let (c, o) = IbcScheme::commit(&pp, 42, 50_000, &mut rng);
        let proof = VVerProver::prove(&pp, &o, &c, 50_000, &mut rng);
        assert!(!VVerVerifier::verify(&pp, &c, 99_900, &proof));
    }
}
