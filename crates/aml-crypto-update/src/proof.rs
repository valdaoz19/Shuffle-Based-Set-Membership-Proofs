use curve25519_dalek::{RistrettoPoint, Scalar};
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};

use aml_crypto_ibc::{IbcCommitment, IbcOpening, IbcScheme, PublicParams};

/// Sigma-protocol proof that `updated = original + g_val^delta` (value-only update).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProof {
    pub updated: IbcCommitment,
    pub t: RistrettoPoint,
    pub z: Scalar,
    pub challenge: Scalar,
    pub delta_value: u64,
}

impl UpdateProof {
    pub fn prove<R: RngCore + CryptoRng>(
        pp: &PublicParams,
        original: &IbcCommitment,
        opening: &IbcOpening,
        add_value: u64,
        rng: &mut R,
    ) -> (IbcCommitment, Self) {
        let updated_opening = IbcOpening {
            id: opening.id,
            value: opening.value.saturating_add(add_value),
            randomness: opening.randomness,
        };
        let updated = IbcScheme::commit_with_randomness(
            pp,
            updated_opening.id,
            updated_opening.value,
            &updated_opening.randomness,
        );

        let alpha = Scalar::random(rng);
        let t = pp.g_val * alpha;
        let beta = Self::derive_challenge(pp, original, &updated, add_value, &t);
        let z = alpha + beta * Scalar::from(add_value);

        let proof = UpdateProof {
            updated,
            t,
            z,
            challenge: beta,
            delta_value: add_value,
        };

        (updated, proof)
    }

    pub fn verify(&self, pp: &PublicParams, original: &IbcCommitment) -> bool {
        let delta_point = pp.g_val * Scalar::from(self.delta_value);
        if self.updated.0 != original.0 + delta_point {
            return false;
        }

        let expected =
            Self::derive_challenge(pp, original, &self.updated, self.delta_value, &self.t);
        if self.challenge != expected {
            return false;
        }

        let diff = self.updated.0 - original.0;
        let lhs = pp.g_val * self.z;
        let rhs = self.t + diff * self.challenge;
        lhs == rhs
    }

    pub fn to_json(&self) -> aml_core::AmlResult<String> {
        serde_json::to_string(self).map_err(|e| aml_core::AmlError::Serialization(e.to_string()))
    }

    fn derive_challenge(
        pp: &PublicParams,
        original: &IbcCommitment,
        updated: &IbcCommitment,
        delta: u64,
        t: &RistrettoPoint,
    ) -> Scalar {
        use sha3::{Digest, Sha3_512};
        let mut hasher = Sha3_512::new();
        hasher.update(b"aml-oblivious-update-v1");
        hasher.update(pp.g_val.compress().as_bytes());
        hasher.update(original.0.compress().as_bytes());
        hasher.update(updated.0.compress().as_bytes());
        hasher.update(delta.to_le_bytes());
        hasher.update(t.compress().as_bytes());
        Scalar::from_hash(hasher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn update_proof_verifies() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let (c, o) = IbcScheme::commit(&pp, 42, 1000, &mut rng);
        let (_updated, proof) = UpdateProof::prove(&pp, &c, &o, 500, &mut rng);
        assert!(proof.verify(&pp, &c));
        assert_eq!(proof.delta_value, 500);
    }
}
