//! Phase 1 coordinate openings; the index is public in BOTH opening modes.
pub mod cfh;
pub mod ep;
use anyhow::{Result, ensure};
use ark_ec::CurveGroup;
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use smp_primitives::*;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Scheme {
    EpUncompressed,
    EpCompressed,
    CfH,
}
impl Scheme {
    pub const ALL: [Self; 3] = [Self::EpUncompressed, Self::EpCompressed, Self::CfH];
    pub fn name(self) -> &'static str {
        match self {
            Self::EpUncompressed => "ep-uncompressed",
            Self::EpCompressed => "ep-compressed",
            Self::CfH => "cf-h",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OpeningMode {
    Plain,
    Hidden,
}
impl OpeningMode {
    pub const ALL: [Self; 2] = [Self::Plain, Self::Hidden];
    pub fn name(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Hidden => "hidden",
        }
    }
}

#[derive(Clone)]
pub struct Statement {
    pub commitment: G1Affine,
    pub index: usize,
    pub claim: Claim,
}
#[derive(Clone)]
pub enum Claim {
    Plain(Fr),
    Hidden(G1Affine),
}
pub struct PlainStatement {
    pub commitment: G1Affine,
    pub index: usize,
    pub value: Fr,
}
pub struct HiddenStatement {
    pub commitment: G1Affine,
    pub index: usize,
    pub value_commitment: G1Affine,
}
pub struct HiddenWitness {
    pub value: Fr,
    pub randomness: Fr,
}
impl From<&PlainStatement> for Statement {
    fn from(s: &PlainStatement) -> Self {
        Self {
            commitment: s.commitment,
            index: s.index,
            claim: Claim::Plain(s.value),
        }
    }
}
impl From<&HiddenStatement> for Statement {
    fn from(s: &HiddenStatement) -> Self {
        Self {
            commitment: s.commitment,
            index: s.index,
            claim: Claim::Hidden(s.value_commitment),
        }
    }
}
impl Statement {
    pub fn mode(&self) -> OpeningMode {
        match self.claim {
            Claim::Plain(_) => OpeningMode::Plain,
            Claim::Hidden(_) => OpeningMode::Hidden,
        }
    }
    pub fn bytes(&self) -> Vec<u8> {
        let mut bytes = encode(&self.commitment);
        bytes.extend_from_slice(&(self.index as u64).to_le_bytes());
        match self.claim {
            Claim::Plain(x) => bytes.extend(encode(&x)),
            Claim::Hidden(c) => bytes.extend(encode(&c)),
        };
        bytes
    }
}

pub fn transcript(scheme: Scheme, parameter_digest: &[u8; 32], s: &Statement) -> Transcript {
    let mut t = Transcript::new(b"SMP-E1-OPENING-v1");
    t.append(b"scheme", scheme.name().as_bytes());
    t.append(b"mode", s.mode().name().as_bytes());
    t.append(b"parameters", parameter_digest);
    t.append(b"statement", &s.bytes());
    t
}

pub enum Parameters {
    Ep(Box<ep::Parameters>),
    CfH(Box<cfh::Parameters>),
}
pub enum OpeningData<'a> {
    Ep { values: &'a [Fr], blind: Fr },
    CfH(G1Affine),
}
impl Parameters {
    pub fn prove_plain<R: RngCore + CryptoRng>(
        &self,
        s: &PlainStatement,
        data: &OpeningData<'_>,
        rng: &mut R,
    ) -> Result<Vec<u8>> {
        self.prove(&s.into(), data, s.value, Fr::from(0), rng)
    }
    pub fn verify_plain(&self, s: &PlainStatement, proof: &[u8]) -> Result<bool> {
        self.verify(&s.into(), proof)
    }
    pub fn prove_hidden<R: RngCore + CryptoRng>(
        &self,
        s: &HiddenStatement,
        data: &OpeningData<'_>,
        w: &HiddenWitness,
        rng: &mut R,
    ) -> Result<Vec<u8>> {
        self.prove(&s.into(), data, w.value, w.randomness, rng)
    }
    pub fn verify_hidden(&self, s: &HiddenStatement, proof: &[u8]) -> Result<bool> {
        self.verify(&s.into(), proof)
    }
    pub fn setup<R: RngCore + CryptoRng>(scheme: Scheme, n: usize, rng: &mut R) -> Result<Self> {
        ensure!(n > 0 && n <= 4096, "n must be in 1..=4096");
        Ok(match scheme {
            Scheme::CfH => Self::CfH(Box::new(cfh::Parameters::setup(n, rng))),
            _ => Self::Ep(Box::new(ep::Parameters::setup(scheme, n))),
        })
    }
    pub fn n(&self) -> usize {
        match self {
            Self::Ep(p) => p.n,
            Self::CfH(p) => p.n,
        }
    }
    pub fn bytes(&self) -> Vec<u8> {
        match self {
            Self::Ep(p) => p.bytes(),
            Self::CfH(p) => p.bytes(),
        }
    }
    pub fn commit(&self, values: &[Fr], blind: Fr) -> Result<G1Affine> {
        ensure!(values.len() == self.n(), "vector length");
        Ok(match self {
            Self::Ep(p) => p.commit(values, blind),
            Self::CfH(p) => p.commit(values, blind),
        })
    }
    pub fn opening_data<'a>(
        &self,
        values: &'a [Fr],
        blind: Fr,
        k: usize,
    ) -> Result<OpeningData<'a>> {
        ensure!(
            values.len() == self.n() && k < self.n(),
            "invalid opening input"
        );
        Ok(match self {
            Self::Ep(_) => OpeningData::Ep { values, blind },
            Self::CfH(p) => OpeningData::CfH(p.opening(values, blind, k)),
        })
    }
    pub fn prove<R: RngCore + CryptoRng>(
        &self,
        s: &Statement,
        data: &OpeningData<'_>,
        x: Fr,
        r: Fr,
        rng: &mut R,
    ) -> Result<Vec<u8>> {
        ensure!(s.index < self.n(), "index out of range");
        match (self, data) {
            (Self::Ep(p), OpeningData::Ep { values, blind }) => p.prove(s, values, *blind, r, rng),
            (Self::CfH(p), OpeningData::CfH(lambda)) => p.prove(s, *lambda, x, r, rng),
            _ => anyhow::bail!("opening data belongs to a different scheme"),
        }
    }
    pub fn verify(&self, s: &Statement, proof: &[u8]) -> Result<bool> {
        ensure!(s.index < self.n(), "index out of range");
        match self {
            Self::Ep(p) => p.verify(s, proof),
            Self::CfH(p) => p.verify(s, proof),
        }
    }
}

pub(crate) fn point(p: G1Projective) -> G1Affine {
    p.into_affine()
}
