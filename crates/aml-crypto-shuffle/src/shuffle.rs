use curve25519_dalek::Scalar;
use rand::seq::SliceRandom;
use rand_core::{CryptoRng, RngCore};
use sha3::{Digest, Sha3_256};

use aml_crypto_ibc::{IbcCommitment, IbcOpening, IbcScheme, PublicParams};

use crate::proof::{RerandomizationProof, ShuffleProof};

pub struct VerifiableShuffle;

impl VerifiableShuffle {
    /// Apply secret permutation and re-randomize each commitment.
    pub fn shuffle<R: RngCore + CryptoRng>(
        pp: &PublicParams,
        inputs: &[IbcCommitment],
        openings: &[IbcOpening],
        perm: &[usize],
        rng: &mut R,
    ) -> (Vec<IbcCommitment>, Vec<Scalar>) {
        assert_eq!(inputs.len(), openings.len());
        assert_eq!(inputs.len(), perm.len());

        let mut outputs = Vec::with_capacity(inputs.len());
        let mut deltas = Vec::with_capacity(inputs.len());

        for (out_idx, &in_idx) in perm.iter().enumerate() {
            let (out, delta) = IbcScheme::rerandomize(pp, &inputs[in_idx], rng);
            // Preserve opening consistency for prover-side checks
            let _expected_opening = IbcOpening {
                id: openings[in_idx].id,
                value: openings[in_idx].value,
                randomness: openings[in_idx].randomness + delta,
            };
            let _ = out_idx;
            outputs.push(out);
            deltas.push(delta);
        }

        (outputs, deltas)
    }

    pub fn random_permutation<R: RngCore + CryptoRng>(n: usize, rng: &mut R) -> Vec<usize> {
        let mut perm: Vec<usize> = (0..n).collect();
        perm.shuffle(rng);
        perm
    }

    pub fn prove<R: RngCore + CryptoRng>(
        pp: &PublicParams,
        inputs: &[IbcCommitment],
        outputs: &[IbcCommitment],
        perm: &[usize],
        deltas: &[Scalar],
        rng: &mut R,
    ) -> ShuffleProof {
        assert_eq!(outputs.len(), perm.len());
        assert_eq!(deltas.len(), perm.len());

        let mut links = Vec::with_capacity(outputs.len());
        for (out_i, (&in_j, delta)) in perm.iter().zip(deltas.iter()).enumerate() {
            let proof = RerandomizationProof::prove(pp, &inputs[in_j], &outputs[out_i], delta, rng);
            links.push(proof);
        }

        let permutation_digest = Self::digest_permutation(perm);

        ShuffleProof {
            outputs: outputs.to_vec(),
            links,
            permutation_digest,
        }
    }

    pub fn verify(pp: &PublicParams, inputs: &[IbcCommitment], proof: &ShuffleProof) -> bool {
        if proof.outputs.len() != proof.links.len() {
            return false;
        }
        if proof.outputs.len() != inputs.len() {
            return false;
        }

        // Verify each output is a valid rerandomization of some input.
        // PoC: verify links against a canonical assignment found by matching.
        let mut used = vec![false; inputs.len()];
        for (out_i, link) in proof.outputs.iter().zip(proof.links.iter()) {
            let mut matched = false;
            for (in_j, input) in inputs.iter().enumerate() {
                if used[in_j] {
                    continue;
                }
                if link.verify(pp, input, out_i) {
                    used[in_j] = true;
                    matched = true;
                    break;
                }
            }
            if !matched {
                return false;
            }
        }
        used.iter().all(|u| *u)
    }

    pub fn shuffle_and_prove<R: RngCore + CryptoRng>(
        pp: &PublicParams,
        inputs: &[IbcCommitment],
        openings: &[IbcOpening],
        rng: &mut R,
    ) -> (ShuffleProof, Vec<IbcOpening>) {
        let perm = Self::random_permutation(inputs.len(), rng);
        let mut outputs = Vec::with_capacity(inputs.len());
        let mut deltas = Vec::with_capacity(inputs.len());
        let mut permuted_openings = Vec::with_capacity(inputs.len());

        for &in_idx in &perm {
            let (out, delta) = IbcScheme::rerandomize(pp, &inputs[in_idx], rng);
            outputs.push(out);
            deltas.push(delta);
            permuted_openings.push(IbcOpening {
                id: openings[in_idx].id,
                value: openings[in_idx].value,
                randomness: openings[in_idx].randomness + delta,
            });
        }

        let proof = Self::prove(pp, inputs, &outputs, &perm, &deltas, rng);
        (proof, permuted_openings)
    }

    fn digest_permutation(perm: &[usize]) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(b"aml-shuffle-perm-v1");
        for p in perm {
            hasher.update(p.to_le_bytes());
        }
        hasher.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn shuffle_proof_verifies() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let mut inputs = Vec::new();
        let mut openings = Vec::new();
        for i in 0..5u64 {
            let (c, o) = IbcScheme::commit(&pp, 42, i * 100, &mut rng);
            inputs.push(c);
            openings.push(o);
        }

        let (proof, permuted_openings) =
            VerifiableShuffle::shuffle_and_prove(&pp, &inputs, &openings, &mut rng);
        assert!(VerifiableShuffle::verify(&pp, &inputs, &proof));
        assert_eq!(permuted_openings.len(), inputs.len());
    }
}
