//! Rev7 Table 7 / Box 3, with independent verification exponents.
use crate::*;
use ark_ec::{
    AffineRepr, Group,
    pairing::{Pairing, PairingOutput},
};
use ark_ff::{PrimeField, UniformRand, Zero};
use zeroize::Zeroize;

pub struct Parameters {
    pub n: usize,
    pub u: Vec<G1Affine>,
    pub hats: Vec<G2Affine>,
    // Row k stores l=0..=n except l=k: the missing diagonal is NEVER published.
    pub cross: Vec<Vec<G1Affine>>,
    pub digest: [u8; 32],
    pub encoding_ns: u128,
    g: G1Affine,
    h: G1Affine,
    e: PairingOutput<Bls12_381>,
}
impl Parameters {
    pub fn setup<R: RngCore + CryptoRng>(n: usize, rng: &mut R) -> Self {
        let mut z: Vec<_> = (0..=n).map(|_| nonzero(rng)).collect();
        let mut y: Vec<_> = (0..n).map(|_| nonzero(rng)).collect();
        let u: Vec<_> = z
            .iter()
            .map(|s| point(G1Projective::generator() * s))
            .collect();
        let hats = y
            .iter()
            .map(|s| (G2Projective::generator() * s).into_affine())
            .collect();
        let cross = y
            .iter()
            .enumerate()
            .map(|(k, yk)| {
                u.iter()
                    .enumerate()
                    .filter(|(l, _)| *l != k)
                    .map(|(_, ul)| point(*ul * yk))
                    .collect()
            })
            .collect();
        let (g, h) = pedersen_bases();
        z.zeroize();
        y.zeroize();
        let e = Bls12_381::pairing(G1Affine::generator(), G2Affine::generator());
        let mut p = Self {
            n,
            u,
            hats,
            cross,
            digest: [0; 32],
            encoding_ns: 0,
            g,
            h,
            e,
        };
        let start = std::time::Instant::now();
        p.digest = digest(&p.bytes());
        p.encoding_ns = start.elapsed().as_nanos();
        // Trapdoor values are scoped to setup and never part of Parameters.
        p
    }
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = b"SMP-CFH-PP-v1".to_vec();
        b.extend((self.n as u64).to_le_bytes());
        b.extend(encode(&G1Affine::generator()));
        b.extend(encode(&G2Affine::generator()));
        b.extend(encode(&self.g));
        b.extend(encode(&self.h));
        for p in &self.u {
            b.extend(encode(p));
        }
        for p in &self.hats {
            b.extend(encode(p));
        }
        for row in &self.cross {
            for p in row {
                b.extend(encode(p));
            }
        }
        b
    }
    pub fn commit(&self, values: &[Fr], blind: Fr) -> G1Affine {
        let mut scalars = values.to_vec();
        scalars.push(blind);
        point(msm(&self.u, &scalars))
    }
    pub fn opening(&self, values: &[Fr], blind: Fr, k: usize) -> G1Affine {
        let mut scalars: Vec<_> = values
            .iter()
            .enumerate()
            .filter(|(l, _)| *l != k)
            .map(|(_, v)| *v)
            .collect();
        scalars.push(blind);
        point(msm(&self.cross[k], &scalars))
    }
    pub fn rerandomize(
        &self,
        r: G1Affine,
        lambda: G1Affine,
        k: usize,
        delta: Fr,
    ) -> (G1Affine, G1Affine) {
        (
            point(r + self.u[self.n] * delta),
            point(lambda + self.cross[k][self.n - 1] * delta),
        )
    }
    pub fn prove<R: RngCore + CryptoRng>(
        &self,
        s: &Statement,
        lambda: G1Affine,
        x: Fr,
        r: Fr,
        rng: &mut R,
    ) -> Result<Vec<u8>> {
        if matches!(s.claim, Claim::Plain(_)) {
            return Ok(proof_encode(&lambda));
        }
        let beta = Fr::rand(rng);
        let a = Fr::rand(rng);
        let b = Fr::rand(rng);
        let t = Fr::rand(rng);
        let theta = point(lambda + G1Projective::generator() * beta);
        let ek = Bls12_381::pairing(self.u[s.index], self.hats[s.index]);
        let a1 = ek * a - self.e * b;
        let a2 = point(self.g * a + self.h * t);
        let mut tr = transcript(Scheme::CfH, &self.digest, s);
        tr.append(b"theta", &encode(&theta));
        tr.append(b"A1-GT", &encode(&a1));
        tr.append(b"A2", &encode(&a2));
        let c = tr.scalar(b"challenge");
        let mut proof = proof_encode(&theta);
        for v in [c, a + c * x, b + c * beta, t + c * r] {
            proof.extend(proof_encode(&v));
        }
        Ok(proof)
    }
    pub fn verify(&self, s: &Statement, proof: &[u8]) -> Result<bool> {
        self.verify_inner(s, proof, true)
    }
    pub fn verify_reference(&self, s: &Statement, proof: &[u8]) -> Result<bool> {
        self.verify_inner(s, proof, false)
    }
    fn verify_inner(&self, s: &Statement, proof: &[u8], multi: bool) -> Result<bool> {
        ensure!(s.index < self.n, "index out of range");
        match s.claim {
            Claim::Plain(x) => {
                if !multi {
                    return self.verify_plain_reference(s, proof);
                }
                ensure!(proof.len() == 48, "CF-H plain size");
                let lambda: G1Affine = decode(proof)?;
                Ok(Bls12_381::multi_pairing(
                    [
                        point(s.commitment.into_group() - self.u[s.index] * x),
                        -lambda,
                    ],
                    [self.hats[s.index], G2Affine::generator()],
                )
                .is_zero())
            }
            Claim::Hidden(cmt) => {
                ensure!(proof.len() == 176, "CF-H hidden size");
                let theta: G1Affine = decode(&proof[..48])?;
                let c: Fr = decode(&proof[48..80])?;
                let z1: Fr = decode(&proof[80..112])?;
                let z2: Fr = decode(&proof[112..144])?;
                let z3: Fr = decode(&proof[144..])?;
                let ek = Bls12_381::pairing(self.u[s.index], self.hats[s.index]);
                let m = if multi {
                    Bls12_381::multi_pairing(
                        [s.commitment, -theta],
                        [self.hats[s.index], G2Affine::generator()],
                    )
                } else {
                    Bls12_381::pairing(s.commitment, self.hats[s.index])
                        - Bls12_381::pairing(theta, G2Affine::generator())
                };
                let a1 = ek * z1 - self.e * z2 - m * c;
                let a2 = point(self.g * z1 + self.h * z3 - cmt * c);
                let mut tr = transcript(Scheme::CfH, &self.digest, s);
                tr.append(b"theta", &encode(&theta));
                tr.append(b"A1-GT", &encode(&a1));
                tr.append(b"A2", &encode(&a2));
                Ok(c == tr.scalar(b"challenge"))
            }
        }
    }
    pub fn verify_plain_reference(&self, s: &Statement, proof: &[u8]) -> Result<bool> {
        let Claim::Plain(x) = s.claim else {
            anyhow::bail!("plain required")
        };
        ensure!(s.index < self.n && proof.len() == 48, "invalid input");
        let lambda: G1Affine = decode(proof)?;
        Ok(Bls12_381::pairing(
            point(s.commitment.into_group() - self.u[s.index] * x),
            self.hats[s.index],
        ) == Bls12_381::pairing(lambda, G2Affine::generator()))
    }
}

pub fn gt_exp(value: PairingOutput<Bls12_381>, scalar: Fr) -> PairingOutput<Bls12_381> {
    value.mul_bigint(scalar.into_bigint())
}
