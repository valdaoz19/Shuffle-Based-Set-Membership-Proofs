use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use cklr_service::method_nnp_v2 as nnp;
use curve25519_dalek::RistrettoPoint;
use curve25519_dalek::ristretto::CompressedRistretto;
use num_bigint::BigInt;

use crate::error::PersephoneError;
use crate::types::{CommitmentState, ValueVerifier};

pub struct PersephoneVerifier;

impl PersephoneVerifier {
    pub fn new() -> Self {
        Self
    }

    fn encode_point(point: &RistrettoPoint) -> String {
        URL_SAFE_NO_PAD.encode(point.compress().as_bytes())
    }

    fn decode_point(encoded: &str) -> Result<RistrettoPoint, PersephoneError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|e| PersephoneError::Encoding(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(PersephoneError::Encoding("bad point length".into()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        let compressed = CompressedRistretto::from_slice(&arr)
            .map_err(|e| PersephoneError::Encoding(e.to_string()))?;
        compressed
            .decompress()
            .ok_or_else(|| PersephoneError::Encoding("invalid point".into()))
    }

    fn parse_r(encoded: &str) -> Result<BigInt, PersephoneError> {
        BigInt::parse_bytes(encoded.as_bytes(), 10)
            .ok_or_else(|| PersephoneError::InvalidState("bad randomness encoding".into()))
    }
}

impl Default for PersephoneVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueVerifier for PersephoneVerifier {
    fn init_bounds(&self, lower: u64, upper: u64) -> Result<CommitmentState, PersephoneError> {
        if lower >= upper {
            return Err(PersephoneError::RangeError(
                "lower must be less than upper".into(),
            ));
        }
        let (_ok, _c_l, _r_l, c_u, r_u, _bench, _comm) = nnp::init_exp(lower, upper);
        Ok(CommitmentState {
            c_l: String::new(),
            r_l: String::new(),
            c_u: Self::encode_point(&c_u),
            r_u: r_u.to_string(),
            lower,
            upper,
            remaining: upper - lower,
        })
    }

    fn prove_transaction(
        &self,
        _lower: u64,
        amount: u64,
        _upper: u64,
        state: &CommitmentState,
    ) -> Result<(bool, CommitmentState), PersephoneError> {
        if amount > state.remaining {
            return Ok((false, state.clone()));
        }

        let c_u = Self::decode_point(&state.c_u)?;
        let r_u = Self::parse_r(&state.r_u)?;

        let (ok, new_c_u, new_r_u, _ms) = nnp::run(amount, state.remaining, c_u, &r_u);

        Ok((
            ok,
            CommitmentState {
                c_l: state.c_l.clone(),
                r_l: state.r_l.clone(),
                c_u: Self::encode_point(&new_c_u),
                r_u: new_r_u.to_string(),
                lower: state.lower,
                upper: state.upper,
                remaining: state.remaining.saturating_sub(amount),
            },
        ))
    }

    fn prove_threshold_headroom(
        &self,
        threshold: u64,
        window_total: u64,
        new_amount: u64,
        state: &CommitmentState,
    ) -> Result<(bool, CommitmentState), PersephoneError> {
        let total_with_new = window_total.saturating_add(new_amount);
        if total_with_new > threshold {
            return Ok((false, state.clone()));
        }

        let remaining = threshold - total_with_new;
        if new_amount > state.remaining {
            return Ok((false, state.clone()));
        }

        let c_u = Self::decode_point(&state.c_u)?;
        let r_u = Self::parse_r(&state.r_u)?;

        // Prove new_amount <= remaining headroom (ZK-MNNP via persephone run).
        let (ok, new_c_u, new_r_u, _ms) = nnp::run(new_amount, state.remaining, c_u, &r_u);

        Ok((
            ok,
            CommitmentState {
                c_l: state.c_l.clone(),
                r_l: state.r_l.clone(),
                c_u: Self::encode_point(&new_c_u),
                r_u: new_r_u.to_string(),
                lower: state.lower,
                upper: threshold,
                remaining,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_three_transactions() {
        let verifier = PersephoneVerifier::new();
        let threshold = 2_000_000u64;
        let mut state = verifier.init_bounds(0, threshold).unwrap();

        let amounts = [500_000u64, 700_000u64, 300_000u64];
        for amount in amounts {
            let (ok, new_state) = verifier
                .prove_transaction(0, amount, threshold, &state)
                .unwrap();
            assert!(ok, "transaction proof should verify");
            state = new_state;
        }

        assert_eq!(state.remaining, threshold - 1_500_000);
        let (ok, _) = verifier
            .prove_threshold_headroom(threshold, 1_500_000, 0, &state)
            .unwrap();
        assert!(ok);
    }

    #[test]
    fn threshold_rejects_overflow() {
        let verifier = PersephoneVerifier::new();
        let threshold = 10_000u64;
        let state = verifier.init_bounds(0, threshold).unwrap();
        let (ok, _) = verifier
            .prove_threshold_headroom(threshold, 9_000, 2_000, &state)
            .unwrap();
        assert!(!ok);
    }
}
