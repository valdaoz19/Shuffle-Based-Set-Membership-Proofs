//! Protocol 1: Π.IdEq — prove two commitments share the same identity.

use curve25519_dalek::{RistrettoPoint, Scalar};
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::commitment::{IbcCommitment, IbcOpening};
use crate::params::PublicParams;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdEqProof {
    pub t: RistrettoPoint,
    pub z_val: Scalar,
    pub z_ran: Scalar,
    pub challenge: Scalar,
}

pub struct IdEqProver;

impl IdEqProver {
    /// Generate proof that id0 == id1 for commitments c0, c1.
    pub fn prove<R: RngCore + CryptoRng>(
        pp: &PublicParams,
        open0: &IbcOpening,
        open1: &IbcOpening,
        c0: &IbcCommitment,
        c1: &IbcCommitment,
        rng: &mut R,
    ) -> IdEqProof {
        assert_eq!(open0.id, open1.id, "IdEq requires equal identities");

        let alpha_val = Scalar::random(rng);
        let alpha_ran = Scalar::random(rng);
        let t = pp.g_val * alpha_val + pp.g_ran * alpha_ran;

        let beta = Self::derive_challenge(pp, c0, c1, &t);

        let v_diff = Scalar::from(open0.value) - Scalar::from(open1.value);
        let r_diff = open0.randomness - open1.randomness;
        let z_val = beta * v_diff + alpha_val;
        let z_ran = beta * r_diff + alpha_ran;

        IdEqProof {
            t,
            z_val,
            z_ran,
            challenge: beta,
        }
    }

    fn derive_challenge(
        pp: &PublicParams,
        c0: &IbcCommitment,
        c1: &IbcCommitment,
        t: &RistrettoPoint,
    ) -> Scalar {
        use sha3::{Digest, Sha3_512};
        let mut hasher = Sha3_512::new();
        hasher.update(b"aml-ibc-ideq-v1");
        hasher.update(pp.g_iden.compress().as_bytes());
        hasher.update(pp.g_val.compress().as_bytes());
        hasher.update(pp.g_ran.compress().as_bytes());
        hasher.update(c0.0.compress().as_bytes());
        hasher.update(c1.0.compress().as_bytes());
        hasher.update(t.compress().as_bytes());
        Scalar::from_hash(hasher)
    }
}

pub struct IdEqVerifier;

impl IdEqVerifier {
    pub fn verify(
        pp: &PublicParams,
        c0: &IbcCommitment,
        c1: &IbcCommitment,
        proof: &IdEqProof,
    ) -> bool {
        let expected_beta = IdEqProver::derive_challenge(pp, c0, c1, &proof.t);
        if proof.challenge != expected_beta {
            return false;
        }

        let lhs = pp.g_val * proof.z_val + pp.g_ran * proof.z_ran;
        let ratio = c0.0 - c1.0;
        let rhs = proof.t + ratio * proof.challenge;
        lhs == rhs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commitment::IbcScheme;
    use rand::rngs::OsRng;

    #[test]
    fn ideq_accepts_same_id() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let (c0, o0) = IbcScheme::commit(&pp, 42, 100, &mut rng);
        let (c1, o1) = IbcScheme::commit(&pp, 42, 200, &mut rng);
        let proof = IdEqProver::prove(&pp, &o0, &o1, &c0, &c1, &mut rng);
        assert!(IdEqVerifier::verify(&pp, &c0, &c1, &proof));
    }

    #[test]
    fn ideq_rejects_different_id() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let (c0, o0) = IbcScheme::commit(&pp, 42, 100, &mut rng);
        let (c1, _o1) = IbcScheme::commit(&pp, 99, 100, &mut rng);
        // Prover would panic on unequal ids; simulate bad proof by swapping
        let proof = IdEqProver::prove(&pp, &o0, &o0, &c0, &c0, &mut rng);
        assert!(!IdEqVerifier::verify(&pp, &c0, &c1, &proof));
    }
}
