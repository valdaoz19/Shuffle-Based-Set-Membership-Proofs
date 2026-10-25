//! EP Sigma and Attema-Cramer Protocol 5 linear-form compression.
use crate::*;
use ark_ec::AffineRepr;
use ark_ff::{One, UniformRand, Zero};

pub struct Parameters {
    pub n: usize,
    pub scheme: Scheme,
    pub bases: Vec<G1Affine>,
    pub digest: [u8; 32],
    pub encoding_ns: u128,
    padding: Vec<G1Affine>,
    k: G1Affine,
    g: G1Affine,
    h: G1Affine,
}
impl Parameters {
    pub fn setup(scheme: Scheme, n: usize) -> Self {
        let domain = b"SMP-E1-EP-BLS12381G1_XMD:SHA-256_SSWU_RO_V1";
        let bases = (0..=n).map(|i| generator(domain, i)).collect();
        let padding = if scheme == Scheme::EpCompressed {
            (n + 1..(n + 1).next_power_of_two())
                .map(|i| generator(domain, i))
                .collect()
        } else {
            Vec::new()
        };
        let k = if scheme == Scheme::EpCompressed {
            generator(b"SMP-E1-EP-COMPRESSION-K-v1", 0)
        } else {
            G1Affine::zero()
        };
        let (g, h) = pedersen_bases();
        let mut p = Self {
            n,
            scheme,
            bases,
            digest: [0; 32],
            padding,
            k,
            g,
            h,
            encoding_ns: 0,
        };
        let now = std::time::Instant::now();
        p.digest = digest(&p.bytes());
        p.encoding_ns = now.elapsed().as_nanos();
        p
    }
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = b"SMP-EP-PP-v1".to_vec();
        b.extend((self.n as u64).to_le_bytes());
        for p in self
            .bases
            .iter()
            .chain(&self.padding)
            .chain([&self.g, &self.h])
        {
            b.extend(encode(p));
        }
        if self.scheme == Scheme::EpCompressed {
            b.extend(encode(&self.k));
        }
        b
    }
    pub fn commit(&self, v: &[Fr], blind: Fr) -> G1Affine {
        let mut w = v.to_vec();
        w.push(blind);
        point(msm(&self.bases, &w))
    }
    pub fn proof_size(&self, mode: OpeningMode) -> usize {
        if self.scheme == Scheme::EpUncompressed {
            32 * (self.n + if mode == OpeningMode::Plain { 1 } else { 3 })
        } else {
            let rounds = (self.n + 1).next_power_of_two().ilog2() as usize - 1;
            2 * rounds * 48 + 64 + if mode == OpeningMode::Plain { 80 } else { 160 }
        }
    }
    pub fn prove<R: RngCore + CryptoRng>(
        &self,
        s: &Statement,
        v: &[Fr],
        blind: Fr,
        r: Fr,
        rng: &mut R,
    ) -> Result<Vec<u8>> {
        ensure!(v.len() == self.n, "vector length");
        if self.scheme == Scheme::EpCompressed {
            return self.prove_compressed(s, v, blind, r, rng);
        }
        let mut bases = self.bases.clone();
        let mut w = v.to_vec();
        w.push(blind);
        let mut tr = transcript(self.scheme, &self.digest, s);
        if let Claim::Plain(_) = s.claim {
            let _ = bases.remove(s.index);
            w.remove(s.index);
        }
        let nonce: Vec<_> = (0..w.len()).map(|_| Fr::rand(rng)).collect();
        tr.append(b"A-R", &encode(&point(msm(&bases, &nonce))));
        let ar = Fr::rand(rng);
        if let Claim::Hidden(_) = s.claim {
            tr.append(
                b"A-C",
                &encode(&point(self.g * nonce[s.index] + self.h * ar)),
            );
        }
        let c = tr.scalar(b"challenge");
        let mut out = proof_encode(&c);
        for (a, x) in nonce.iter().zip(w) {
            out.extend(proof_encode(&(*a + c * x)));
        }
        if let Claim::Hidden(_) = s.claim {
            out.extend(proof_encode(&(ar + c * r)));
        }
        Ok(out)
    }
    pub fn verify(&self, s: &Statement, proof: &[u8]) -> Result<bool> {
        ensure!(proof.len() == self.proof_size(s.mode()), "EP proof size");
        if self.scheme == Scheme::EpCompressed {
            return self.verify_compressed(s, proof, false);
        }
        let (chunks, remainder) = proof.as_chunks::<32>();
        ensure!(remainder.is_empty(), "EP scalar alignment");
        let values: Vec<Fr> = chunks
            .iter()
            .map(|chunk| decode::<Fr>(chunk))
            .collect::<Result<_>>()?;
        let c = values[0];
        let z = &values[1..];
        let mut bases = self.bases.clone();
        let mut tr = transcript(self.scheme, &self.digest, s);
        match s.claim {
            Claim::Plain(x) => {
                let _ = bases.remove(s.index);
                tr.append(
                    b"A-R",
                    &encode(&point(
                        msm(&bases, z) - (s.commitment.into_group() - self.bases[s.index] * x) * c,
                    )),
                );
            }
            Claim::Hidden(cmt) => {
                tr.append(
                    b"A-R",
                    &encode(&point(msm(&bases, &z[..self.n + 1]) - s.commitment * c)),
                );
                tr.append(
                    b"A-C",
                    &encode(&point(
                        self.g * z[s.index] + self.h * z[self.n + 1] - cmt * c,
                    )),
                );
            }
        }
        Ok(c == tr.scalar(b"challenge"))
    }
    fn padded(&self) -> Vec<G1Affine> {
        self.bases.iter().chain(&self.padding).copied().collect()
    }
    fn linear(&self, index: usize, tr: &mut Transcript) -> Vec<Fr> {
        let mut l = vec![Fr::zero(); (self.n + 1).next_power_of_two()];
        l[index] = Fr::one();
        // Batch zero checks for the unused coordinates; sampled after R is bound.
        for x in &mut l[self.n + 1..] {
            *x = tr.scalar(b"padding-nullity");
        }
        l
    }
    fn prove_compressed<R: RngCore + CryptoRng>(
        &self,
        s: &Statement,
        v: &[Fr],
        blind: Fr,
        r: Fr,
        rng: &mut R,
    ) -> Result<Vec<u8>> {
        let mut bases = self.padded();
        let d = bases.len();
        let mut w = v.to_vec();
        w.push(blind);
        w.resize(d, Fr::zero());
        // Padding witnesses and padding masks are zero; independent bases bind them.
        let mut a: Vec<_> = (0..=self.n).map(|_| Fr::rand(rng)).collect();
        a.resize(d, Fr::zero());
        let ar = point(msm(&bases, &a));
        let mut tr = transcript(self.scheme, &self.digest, s);
        let mut out = Vec::new();
        let mut linear = self.linear(s.index, &mut tr);
        let (c, linear_value) = match s.claim {
            Claim::Plain(x) => {
                let t = a[s.index];
                out.extend(proof_encode(&t));
                out.extend(proof_encode(&ar));
                tr.append(b"t", &encode(&t));
                tr.append(b"A-R", &encode(&ar));
                let c = tr.scalar(b"c0");
                (c, c * x + t)
            }
            Claim::Hidden(_) => {
                let eta = Fr::rand(rng);
                let ac = point(self.g * a[s.index] + self.h * eta);
                out.extend(proof_encode(&ar));
                out.extend(proof_encode(&ac));
                tr.append(b"A-R", &encode(&ar));
                tr.append(b"A-C", &encode(&ac));
                let c = tr.scalar(b"c0");
                let z = a[s.index] + c * v[s.index];
                let phi = eta + c * r;
                out.extend(proof_encode(&z));
                out.extend(proof_encode(&phi));
                tr.append(b"z-k", &encode(&z));
                tr.append(b"phi", &encode(&phi));
                (c, z)
            }
        };
        let c1 = tr.scalar(b"c1");
        for x in &mut linear {
            *x *= c1;
        }
        let mut z: Vec<_> = a.iter().zip(w).map(|(a, w)| *a + c * w).collect();
        let _q = ar + s.commitment * c + self.k * (c1 * linear_value);
        while z.len() > 2 {
            let h = z.len() / 2;
            let left = point(msm(&bases[h..], &z[..h]) + self.k * dot(&linear[h..], &z[..h]));
            let right = point(msm(&bases[..h], &z[h..]) + self.k * dot(&linear[..h], &z[h..]));
            out.extend(proof_encode(&left));
            out.extend(proof_encode(&right));
            tr.append(b"fold-left", &encode(&left));
            tr.append(b"fold-right", &encode(&right));
            let c = tr.scalar(b"fold");
            bases = (0..h).map(|i| point(bases[i] * c + bases[h + i])).collect();
            linear = (0..h).map(|i| linear[i] * c + linear[h + i]).collect();
            z = (0..h).map(|i| z[i] + c * z[h + i]).collect();
        }
        out.extend(proof_encode(&z[0]));
        out.extend(proof_encode(&z[1]));
        Ok(out)
    }
    pub fn verify_compressed(&self, s: &Statement, proof: &[u8], reference: bool) -> Result<bool> {
        ensure!(
            s.index < self.n && proof.len() == self.proof_size(s.mode()),
            "invalid compressed proof"
        );
        let mut rd = Reader {
            bytes: proof,
            offset: 0,
        };
        let mut tr = transcript(self.scheme, &self.digest, s);
        let mut linear = self.linear(s.index, &mut tr);
        let (ar, c, linear_value) = match s.claim {
            Claim::Plain(x) => {
                let t: Fr = rd.take(32)?;
                let ar: G1Affine = rd.take(48)?;
                tr.append(b"t", &encode(&t));
                tr.append(b"A-R", &encode(&ar));
                let c = tr.scalar(b"c0");
                (ar, c, c * x + t)
            }
            Claim::Hidden(cmt) => {
                let ar: G1Affine = rd.take(48)?;
                let ac: G1Affine = rd.take(48)?;
                tr.append(b"A-R", &encode(&ar));
                tr.append(b"A-C", &encode(&ac));
                let c = tr.scalar(b"c0");
                let z: Fr = rd.take(32)?;
                let phi: Fr = rd.take(32)?;
                if point(self.g * z + self.h * phi) != point(ac + cmt * c) {
                    return Ok(false);
                }
                tr.append(b"z-k", &encode(&z));
                tr.append(b"phi", &encode(&phi));
                (ar, c, z)
            }
        };
        let c1 = tr.scalar(b"c1");
        let mut q = ar + s.commitment * c + self.k * (c1 * linear_value);
        let original = self.padded();
        let mut bases = original.clone();
        for x in &mut linear {
            *x *= c1;
        }
        let original_linear = linear.clone();
        let mut challenges = Vec::new();
        let mut size = bases.len();
        while size > 2 {
            let a: G1Affine = rd.take(48)?;
            let b: G1Affine = rd.take(48)?;
            tr.append(b"fold-left", &encode(&a));
            tr.append(b"fold-right", &encode(&b));
            let c = tr.scalar(b"fold");
            q = a + q * c + b * (c * c);
            challenges.push(c);
            let h = size / 2;
            if reference {
                bases = (0..h).map(|i| point(bases[i] * c + bases[h + i])).collect();
                linear = (0..h).map(|i| linear[i] * c + linear[h + i]).collect();
            }
            size = h;
        }
        let z0: Fr = rd.take(32)?;
        let z1: Fr = rd.take(32)?;
        if reference {
            return Ok(q == msm(&bases, &[z0, z1]) + self.k * (linear[0] * z0 + linear[1] * z1));
        }
        let mut weights = vec![Fr::one(); original.len()];
        for (round, c) in challenges.iter().enumerate() {
            let half = original.len() >> (round + 1);
            for (i, w) in weights.iter_mut().enumerate() {
                if (i / half).is_multiple_of(2) {
                    *w *= c;
                }
            }
        }
        for (i, w) in weights.iter_mut().enumerate() {
            *w *= if i % 2 == 0 { z0 } else { z1 };
        }
        Ok(q == msm(&original, &weights) + self.k * dot(&original_linear, &weights))
    }
}
fn dot(a: &[Fr], b: &[Fr]) -> Fr {
    a.iter().zip(b).map(|(a, b)| *a * b).sum()
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Reader<'_> {
    fn take<T: CanonicalDeserialize + CanonicalSerialize>(&mut self, n: usize) -> Result<T> {
        let v = decode(&self.bytes[self.offset..self.offset + n])?;
        self.offset += n;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::StdRng};
    #[test]
    fn padding_bases_and_first_messages_are_bound() {
        let mut rng = StdRng::seed_from_u64(45);
        let mut p = Parameters::setup(Scheme::EpCompressed, 4);
        let v = vec![Fr::from(7); 4];
        let s = Statement {
            commitment: p.commit(&v, Fr::from(8)),
            index: 2,
            claim: Claim::Hidden(pedersen(v[2], Fr::from(9))),
        };
        let proof = p.prove(&s, &v, Fr::from(8), Fr::from(9), &mut rng).unwrap();
        assert!(p.verify(&s, &proof).unwrap());
        p.padding[0] = point(p.padding[0] + G1Affine::generator());
        assert!(!p.verify(&s, &proof).unwrap());
    }
}
