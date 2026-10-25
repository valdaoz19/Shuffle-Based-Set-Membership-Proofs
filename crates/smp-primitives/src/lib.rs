//! Canonical BLS12-381 primitives for the isolated E1 experiment.
use anyhow::{Result, ensure};
pub use ark_bls12_381::{Bls12_381, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{
    CurveGroup, VariableBaseMSM,
    hashing::{HashToCurve, curve_maps::wb::WBMap, map_to_curve_hasher::MapToCurveBasedHasher},
};
use ark_ff::{PrimeField, UniformRand, Zero, field_hashers::DefaultFieldHasher};
pub use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use rand::{CryptoRng, RngCore};
use sha2::{Digest, Sha256};
use std::{cell::Cell, time::Instant};

thread_local! {
    static IO_TIMES: Cell<(u128,u128)> = const { Cell::new((0,0)) };
}
pub fn reset_io_times() {
    IO_TIMES.with(|x| x.set((0, 0)));
}
pub fn io_times() -> (u128, u128) {
    IO_TIMES.with(Cell::get)
}
pub fn proof_encode<T: CanonicalSerialize>(value: &T) -> Vec<u8> {
    let t = Instant::now();
    let bytes = encode(value);
    let ns = t.elapsed().as_nanos();
    IO_TIMES.with(|x| {
        let (a, b) = x.get();
        x.set((a + ns, b));
    });
    bytes
}

pub fn encode<T: CanonicalSerialize>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(value.compressed_size());
    value
        .serialize_compressed(&mut bytes)
        .expect("Vec writes cannot fail");
    bytes
}

/// Deserialization validates canonical scalars and curve/subgroup membership.
pub fn decode<T: CanonicalDeserialize + CanonicalSerialize>(bytes: &[u8]) -> Result<T> {
    let start = Instant::now();
    let mut reader = bytes;
    let value = T::deserialize_compressed(&mut reader)?;
    ensure!(reader.is_empty(), "trailing bytes");
    ensure!(encode(&value) == bytes, "noncanonical encoding");
    IO_TIMES.with(|x| {
        let (a, b) = x.get();
        x.set((a, b + start.elapsed().as_nanos()));
    });
    Ok(value)
}

pub fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// RFC 9380-style WB hash-to-curve, never hash-to-scalar times a known generator.
pub fn generator(domain: &[u8], index: usize) -> G1Affine {
    type Hasher = MapToCurveBasedHasher<
        G1Projective,
        DefaultFieldHasher<Sha256, 128>,
        WBMap<ark_bls12_381::g1::Config>,
    >;
    Hasher::new(domain)
        .expect("valid hash suite")
        .hash(&(index as u64).to_be_bytes())
        .expect("hash to curve")
}

pub fn pedersen_bases() -> (G1Affine, G1Affine) {
    (
        generator(b"SMP-E1-PEDERSEN-BLS12381G1_XMD:SHA-256_SSWU_RO_V1", 0),
        generator(b"SMP-E1-PEDERSEN-BLS12381G1_XMD:SHA-256_SSWU_RO_V1", 1),
    )
}

pub fn pedersen(value: Fr, randomness: Fr) -> G1Affine {
    let (g, h) = pedersen_bases();
    msm(&[g, h], &[value, randomness]).into_affine()
}

pub fn msm(bases: &[G1Affine], scalars: &[Fr]) -> G1Projective {
    assert_eq!(bases.len(), scalars.len());
    G1Projective::msm_unchecked(bases, scalars)
}

pub fn nonzero<R: RngCore + CryptoRng>(rng: &mut R) -> Fr {
    loop {
        let s = Fr::rand(rng);
        if !s.is_zero() {
            return s;
        }
    }
}

#[derive(Clone)]
pub struct Transcript(Sha256);
impl Transcript {
    pub fn new(domain: &[u8]) -> Self {
        let mut t = Self(Sha256::new());
        t.append(b"domain", domain);
        t
    }
    pub fn append(&mut self, label: &[u8], bytes: &[u8]) {
        self.0.update((label.len() as u64).to_be_bytes());
        self.0.update(label);
        self.0.update((bytes.len() as u64).to_be_bytes());
        self.0.update(bytes);
    }
    pub fn scalar(&mut self, label: &[u8]) -> Fr {
        // Wide reduction avoids the bias of reducing a single SHA-256 digest.
        let mut wide = [0u8; 64];
        for i in 0..2u8 {
            let mut hash = self.0.clone();
            hash.update((label.len() as u64).to_be_bytes());
            hash.update(label);
            hash.update([i]);
            wide[i as usize * 32..(i as usize + 1) * 32].copy_from_slice(&hash.finalize());
        }
        self.append(label, &wide);
        Fr::from_le_bytes_mod_order(&wide)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_and_separated() {
        let a = generator(b"test-domain", 0);
        assert_ne!(a, generator(b"test-domain", 1));
        assert_eq!(encode(&a).len(), 48);
        assert_eq!(decode::<G1Affine>(&encode(&a)).unwrap(), a);
        assert!(decode::<Fr>(&[255; 32]).is_err());
        let mut bytes = encode(&a);
        bytes.push(0);
        assert!(decode::<G1Affine>(&bytes).is_err());
        assert!(decode::<G1Affine>(&[255; 48]).is_err());
    }
}
