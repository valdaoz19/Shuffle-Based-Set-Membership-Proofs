use curve25519_dalek::{RistrettoPoint, Scalar};
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};

use aml_crypto_ibc::{IbcCommitment, PublicParams};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerandomizationProof {
    pub t: RistrettoPoint,
    pub z: Scalar,
    pub challenge: Scalar,
}

impl RerandomizationProof {
    pub fn prove<R: RngCore + CryptoRng>(
        pp: &PublicParams,
        input: &IbcCommitment,
        output: &IbcCommitment,
        delta: &Scalar,
        rng: &mut R,
    ) -> Self {
        let alpha = Scalar::random(rng);
        let t = pp.g_ran * alpha;
        let beta = Self::derive_challenge(pp, input, output, &t);
        let z = alpha + beta * delta;
        RerandomizationProof {
            t,
            z,
            challenge: beta,
        }
    }

    pub fn verify(&self, pp: &PublicParams, input: &IbcCommitment, output: &IbcCommitment) -> bool {
        let expected = Self::derive_challenge(pp, input, output, &self.t);
        if self.challenge != expected {
            return false;
        }
        let ratio = output.0 - input.0;
        let lhs = pp.g_ran * self.z;
        let rhs = self.t + ratio * self.challenge;
        lhs == rhs
    }

    fn derive_challenge(
        pp: &PublicParams,
        input: &IbcCommitment,
        output: &IbcCommitment,
        t: &RistrettoPoint,
    ) -> Scalar {
        use sha3::{Digest, Sha3_512};
        let mut hasher = Sha3_512::new();
        hasher.update(b"aml-shuffle-rerand-v1");
        hasher.update(pp.g_ran.compress().as_bytes());
        hasher.update(input.0.compress().as_bytes());
        hasher.update(output.0.compress().as_bytes());
        hasher.update(t.compress().as_bytes());
        Scalar::from_hash(hasher)
    }
}

/// Full shuffle proof: outputs + per-element rerandomization links.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShuffleProof {
    pub outputs: Vec<IbcCommitment>,
    pub links: Vec<RerandomizationProof>,
    /// Commitment to permutation (hash-based for PoC audit).
    pub permutation_digest: [u8; 32],
}

impl ShuffleProof {
    pub fn to_json(&self) -> aml_core::AmlResult<String> {
        serde_json::to_string(self).map_err(|e| aml_core::AmlError::Serialization(e.to_string()))
    }

    pub fn from_json(json: &str) -> aml_core::AmlResult<Self> {
        serde_json::from_str(json).map_err(|e| aml_core::AmlError::Serialization(e.to_string()))
    }
}
