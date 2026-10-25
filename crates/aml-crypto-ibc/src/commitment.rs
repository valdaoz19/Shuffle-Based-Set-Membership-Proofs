use curve25519_dalek::ristretto::CompressedRistretto;
use curve25519_dalek::{RistrettoPoint, Scalar};
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::params::PublicParams;

/// Secret opening (id, value, randomness).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbcOpening {
    pub id: u64,
    pub value: u64,
    pub randomness: Scalar,
}

/// Pedersen IBC commitment c = g_iden^id · g_val^v · g_ran^r.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IbcCommitment(pub RistrettoPoint);

pub struct IbcScheme;

impl IbcScheme {
    pub fn commit<R: RngCore + CryptoRng>(
        pp: &PublicParams,
        id: u64,
        value: u64,
        rng: &mut R,
    ) -> (IbcCommitment, IbcOpening) {
        let randomness = Scalar::random(rng);
        let commitment = Self::commit_with_randomness(pp, id, value, &randomness);
        (
            commitment,
            IbcOpening {
                id,
                value,
                randomness,
            },
        )
    }

    pub fn commit_with_randomness(
        pp: &PublicParams,
        id: u64,
        value: u64,
        randomness: &Scalar,
    ) -> IbcCommitment {
        let id_s = Scalar::from(id);
        let val_s = Scalar::from(value);
        let point = pp.g_iden * id_s + pp.g_val * val_s + pp.g_ran * randomness;
        IbcCommitment(point)
    }

    pub fn verify(pp: &PublicParams, commitment: &IbcCommitment, opening: &IbcOpening) -> bool {
        let expected =
            Self::commit_with_randomness(pp, opening.id, opening.value, &opening.randomness);
        expected.0 == commitment.0
    }

    pub fn rerandomize<R: RngCore + CryptoRng>(
        pp: &PublicParams,
        commitment: &IbcCommitment,
        rng: &mut R,
    ) -> (IbcCommitment, Scalar) {
        let delta = Scalar::random(rng);
        (IbcCommitment(commitment.0 + pp.g_ran * delta), delta)
    }

    pub fn encode(commitment: &IbcCommitment) -> aml_core::AmlResult<String> {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        Ok(URL_SAFE_NO_PAD.encode(commitment.0.compress().as_bytes()))
    }

    pub fn decode(encoded: &str) -> aml_core::AmlResult<IbcCommitment> {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|e| aml_core::AmlError::Serialization(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(aml_core::AmlError::Serialization(
                "invalid commitment length".into(),
            ));
        }
        let compressed = CompressedRistretto::from_slice(&bytes)
            .map_err(|e| aml_core::AmlError::Serialization(e.to_string()))?;
        let point = compressed
            .decompress()
            .ok_or_else(|| aml_core::AmlError::Serialization("invalid point".into()))?;
        Ok(IbcCommitment(point))
    }

    pub fn point_sub(a: &IbcCommitment, b: &IbcCommitment) -> RistrettoPoint {
        a.0 - b.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn commit_verify_roundtrip() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let (c, o) = IbcScheme::commit(&pp, 42, 10_000, &mut rng);
        assert!(IbcScheme::verify(&pp, &c, &o));
    }

    #[test]
    fn verify_rejects_wrong_opening() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let (c, mut o) = IbcScheme::commit(&pp, 42, 10_000, &mut rng);
        o.value = 20_000;
        assert!(!IbcScheme::verify(&pp, &c, &o));
    }

    #[test]
    fn encode_decode_roundtrip() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let (c, _) = IbcScheme::commit(&pp, 1, 50, &mut rng);
        let enc = IbcScheme::encode(&c).unwrap();
        let dec = IbcScheme::decode(&enc).unwrap();
        assert_eq!(c, dec);
    }

    #[test]
    fn rerandomize_preserves_id_and_value() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let (c, o) = IbcScheme::commit(&pp, 7, 999, &mut rng);
        let (c2, delta) = IbcScheme::rerandomize(&pp, &c, &mut rng);
        let o2 = IbcOpening {
            id: o.id,
            value: o.value,
            randomness: o.randomness + delta,
        };
        assert!(IbcScheme::verify(&pp, &c2, &o2));
    }
}
