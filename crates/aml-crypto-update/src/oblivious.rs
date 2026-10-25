use rand_core::{CryptoRng, RngCore};

use aml_crypto_ibc::{IbcCommitment, IbcOpening, PublicParams};

use crate::proof::UpdateProof;

pub struct ObliviousUpdate;

impl ObliviousUpdate {
    /// Update commitment at `target_index` by adding `add_value` without revealing index publicly.
    pub fn apply<R: RngCore + CryptoRng>(
        pp: &PublicParams,
        commitments: &mut [IbcCommitment],
        openings: &mut [IbcOpening],
        target_index: usize,
        add_value: u64,
        rng: &mut R,
    ) -> UpdateProof {
        let original = commitments[target_index];
        let opening = &openings[target_index];
        let (updated, proof) = UpdateProof::prove(pp, &original, opening, add_value, rng);
        openings[target_index].value = opening.value.saturating_add(add_value);
        commitments[target_index] = updated;
        proof
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aml_crypto_ibc::IbcScheme;
    use rand::rngs::OsRng;

    #[test]
    fn applies_update_at_index() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let mut commitments = Vec::new();
        let mut openings = Vec::new();
        for v in [100u64, 200, 300] {
            let (c, o) = IbcScheme::commit(&pp, 1, v, &mut rng);
            commitments.push(c);
            openings.push(o);
        }
        let original = commitments[1];
        let proof = ObliviousUpdate::apply(&pp, &mut commitments, &mut openings, 1, 50, &mut rng);
        assert!(proof.verify(&pp, &original));
        assert_eq!(openings[1].value, 250);
    }
}
