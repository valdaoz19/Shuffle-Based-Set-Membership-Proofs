use curve25519_dalek::RistrettoPoint;
use sha3::{Digest, Sha3_512};

/// Derive a nothing-up-my-sleeve Ristretto generator from a domain label.
pub fn generator_from_label(label: &[u8]) -> RistrettoPoint {
    let mut hasher = Sha3_512::new();
    hasher.update(b"aml-zkp-ibc-generator");
    hasher.update(label);
    RistrettoPoint::from_uniform_bytes(&hasher.finalize().into())
}
