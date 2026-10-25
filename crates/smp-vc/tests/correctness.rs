use ark_ec::pairing::Pairing;
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::{Field, UniformRand};
use rand::{SeedableRng, rngs::StdRng};
use smp_primitives::*;
use smp_vc::*;

#[test]
fn all_six_relations_and_negative_inputs() {
    let mut rng = StdRng::seed_from_u64(832);
    for n in [1, 3, 4, 8] {
        for scheme in Scheme::ALL {
            let p = Parameters::setup(scheme, n, &mut rng).unwrap();
            let v: Vec<_> = (0..n).map(|_| Fr::rand(&mut rng)).collect();
            let blind = Fr::rand(&mut rng);
            let commitment = p.commit(&v, blind).unwrap();
            for mode in OpeningMode::ALL {
                for k in [0, n - 1] {
                    let r = Fr::rand(&mut rng);
                    let s = Statement {
                        commitment,
                        index: k,
                        claim: match mode {
                            OpeningMode::Plain => Claim::Plain(v[k]),
                            OpeningMode::Hidden => Claim::Hidden(pedersen(v[k], r)),
                        },
                    };
                    let data = p.opening_data(&v, blind, k).unwrap();
                    let proof = p.prove(&s, &data, v[k], r, &mut rng).unwrap();
                    assert!(p.verify(&s, &proof).unwrap(), "{scheme:?}/{mode:?}/n={n}");
                    let size = match scheme {
                        Scheme::CfH => {
                            if mode == OpeningMode::Plain {
                                48
                            } else {
                                176
                            }
                        }
                        Scheme::EpUncompressed => {
                            32 * (n + if mode == OpeningMode::Plain { 1 } else { 3 })
                        }
                        Scheme::EpCompressed => {
                            let d = (n + 1).next_power_of_two();
                            (2 * d.ilog2() as usize - 1) * 48
                                + 96
                                + if mode == OpeningMode::Hidden { 80 } else { 0 }
                        }
                    };
                    assert_eq!(proof.len(), size);
                    if let Parameters::Ep(ep) = &p
                        && scheme == Scheme::EpCompressed
                    {
                        assert!(ep.verify_compressed(&s, &proof, true).unwrap());
                    }
                    if let Parameters::CfH(cf) = &p {
                        assert!(cf.verify_reference(&s, &proof).unwrap());
                    }
                    let mut wrong = s.clone();
                    wrong.claim = match mode {
                        OpeningMode::Plain => Claim::Plain(v[k] + Fr::from(1)),
                        OpeningMode::Hidden => Claim::Hidden(pedersen(v[k] + Fr::from(1), r)),
                    };
                    assert!(!p.verify(&wrong, &proof).unwrap());
                    if let Parameters::CfH(cf) = &p {
                        assert!(!cf.verify_reference(&wrong, &proof).unwrap());
                    }
                    if let Parameters::Ep(ep) = &p
                        && scheme == Scheme::EpCompressed
                    {
                        assert!(!ep.verify_compressed(&wrong, &proof, true).unwrap());
                    }
                    wrong = s.clone();
                    wrong.commitment = (commitment + G1Affine::generator()).into_affine();
                    assert!(!p.verify(&wrong, &proof).unwrap());
                    wrong = s.clone();
                    wrong.index = n;
                    assert!(p.verify(&wrong, &proof).is_err());
                    if n > 1 {
                        wrong.index = (k + 1) % n;
                        assert!(!p.verify(&wrong, &proof).unwrap());
                    }
                    let mut trailing = proof.clone();
                    trailing.push(0);
                    assert!(p.verify(&s, &trailing).is_err());
                    assert!(p.verify(&s, &proof[..proof.len() - 1]).is_err());
                    for i in [0, proof.len() / 2, proof.len() - 1] {
                        let mut modified = proof.clone();
                        modified[i] ^= 1;
                        assert!(!p.verify(&s, &modified).unwrap_or(false));
                    }
                    assert!(!p.verify(&s, &vec![255; proof.len()]).unwrap_or(false));
                }
            }
        }
    }
}

#[test]
fn uncompressed_ep_rejects_invalid_scalar_layouts() {
    let mut rng = StdRng::seed_from_u64(506);
    let p = Parameters::setup(Scheme::EpUncompressed, 3, &mut rng).unwrap();
    let values = vec![Fr::from(5), Fr::from(7), Fr::from(11)];
    let blind = Fr::from(13);
    let randomness = Fr::from(17);
    let index = 1;
    let commitment = p.commit(&values, blind).unwrap();
    let data = p.opening_data(&values, blind, index).unwrap();

    for mode in OpeningMode::ALL {
        let statement = Statement {
            commitment,
            index,
            claim: match mode {
                OpeningMode::Plain => Claim::Plain(values[index]),
                OpeningMode::Hidden => Claim::Hidden(pedersen(values[index], randomness)),
            },
        };
        let proof = p
            .prove(&statement, &data, values[index], randomness, &mut rng)
            .unwrap();
        assert!(p.verify(&statement, &proof).unwrap());
        let mut extra_scalar = proof.clone();
        extra_scalar.extend(encode(&Fr::from(0)));
        for (case, invalid) in [
            ("empty", Vec::new()),
            ("partial_scalar", proof[..31].to_vec()),
            ("unaligned", proof[..proof.len() - 1].to_vec()),
            ("missing_scalar", proof[..proof.len() - 32].to_vec()),
            ("extra_scalar", extra_scalar),
        ] {
            assert!(
                p.verify(&statement, &invalid).is_err(),
                "{mode:?}/{case} must be rejected"
            );
        }
        let mut noncanonical = proof.clone();
        noncanonical[..32].fill(255);
        assert!(p.verify(&statement, &noncanonical).is_err());
    }
}

#[test]
fn parameters_domain_mode_and_padding_bound() {
    let mut rng = StdRng::seed_from_u64(2);
    for scheme in Scheme::ALL {
        let mut p = Parameters::setup(scheme, 4, &mut rng).unwrap();
        let v = vec![Fr::from(9); 4];
        let s = Statement {
            commitment: p.commit(&v, Fr::from(3)).unwrap(),
            index: 1,
            claim: Claim::Hidden(pedersen(v[1], Fr::from(4))),
        };
        let data = p.opening_data(&v, Fr::from(3), 1).unwrap();
        let proof = p.prove(&s, &data, v[1], Fr::from(4), &mut rng).unwrap();
        match &mut p {
            Parameters::Ep(ep) => ep.digest[0] ^= 1,
            Parameters::CfH(cf) => cf.digest[0] ^= 1,
        };
        assert!(!p.verify(&s, &proof).unwrap());
        let p_other = Parameters::setup(scheme, 8, &mut rng).unwrap();
        assert!(!p_other.verify(&s, &proof).unwrap_or(false));
    }
    let mut a = Transcript::new(b"a");
    let mut b = Transcript::new(b"b");
    assert_ne!(a.scalar(b"c"), b.scalar(b"c"));
    let mut a = Transcript::new(b"a");
    let mut b = a.clone();
    a.append(b"x", b"ab");
    b.append(b"xa", b"b");
    assert_ne!(a.scalar(b"c"), b.scalar(b"c"));
}

#[test]
fn cfh_rerandomization_and_sigma_extraction() {
    let mut rng = StdRng::seed_from_u64(6);
    let p = cfh::Parameters::setup(4, &mut rng);
    let v = vec![Fr::from(9); 4];
    let blind = Fr::from(8);
    let delta = Fr::from(7);
    let (r, l) = p.rerandomize(p.commit(&v, blind), p.opening(&v, blind, 2), 2, delta);
    assert_eq!(r, p.commit(&v, blind + delta));
    assert_eq!(l, p.opening(&v, blind + delta, 2));
    assert!(
        p.verify(
            &Statement {
                commitment: r,
                index: 2,
                claim: Claim::Plain(v[2])
            },
            &encode(&l)
        )
        .unwrap()
    );
    let c = Fr::from(13);
    let cp = Fr::from(17);
    let denom = (c - cp).inverse().unwrap();
    for witness in [v[0], blind, delta] {
        let a = Fr::rand(&mut rng);
        let z = a + c * witness;
        let zp = a + cp * witness;
        assert_eq!((z - zp) * denom, witness);
    }
}

#[test]
fn rejects_on_curve_non_subgroup_point() {
    // Construct an on-curve point before cofactor clearing.
    for i in 0..1000u64 {
        if let Some(p) = G1Affine::get_point_from_x_unchecked(ark_bls12_381::Fq::from(i), false)
            && !p.is_in_correct_subgroup_assuming_on_curve()
        {
            assert!(decode::<G1Affine>(&encode(&p)).is_err());
            return;
        }
    }
    panic!("test must find a non-subgroup point");
}

#[test]
fn cfh_extraction_satisfies_both_group_equations() {
    let mut rng = StdRng::seed_from_u64(12);
    let p = cfh::Parameters::setup(3, &mut rng);
    let v = vec![Fr::from(11), Fr::from(12), Fr::from(13)];
    let blind = Fr::from(8);
    let k = 1;
    let x = v[k];
    let r = Fr::rand(&mut rng);
    let beta = Fr::rand(&mut rng);
    let (a, b, t) = (Fr::rand(&mut rng), Fr::rand(&mut rng), Fr::rand(&mut rng));
    let c = Fr::from(11);
    let cp = Fr::from(29);
    let inv = (c - cp).inverse().unwrap();
    let z = [a + c * x, b + c * beta, t + c * r];
    let zp = [a + cp * x, b + cp * beta, t + cp * r];
    let extracted: Vec<_> = z.iter().zip(zp).map(|(a, b)| (*a - b) * inv).collect();
    let lambda = p.opening(&v, blind, k);
    let theta = (lambda + G1Affine::generator() * beta).into_affine();
    let r_v = p.commit(&v, blind);
    let ek = Bls12_381::pairing(p.u[k], p.hats[k]);
    let e = Bls12_381::pairing(G1Affine::generator(), G2Affine::generator());
    let m = Bls12_381::multi_pairing([r_v, -theta], [p.hats[k], G2Affine::generator()]);
    assert_eq!(m, ek * extracted[0] - e * extracted[1]);
    assert_eq!(pedersen(extracted[0], extracted[2]), pedersen(x, r));
    let recovered = (theta.into_group() - G1Affine::generator() * extracted[1]).into_affine();
    assert!(
        p.verify_plain_reference(
            &Statement {
                commitment: r_v,
                index: k,
                claim: Claim::Plain(extracted[0])
            },
            &encode(&recovered)
        )
        .unwrap()
    );
    assert_eq!(encode(&e).len(), 576);
    assert_eq!(encode(&G2Affine::generator()).len(), 96);
}

#[test]
fn typed_interfaces_agree() {
    let mut rng = StdRng::seed_from_u64(93);
    for scheme in Scheme::ALL {
        let p = Parameters::setup(scheme, 3, &mut rng).unwrap();
        let v = vec![Fr::from(7); 3];
        let data = p.opening_data(&v, Fr::from(2), 0).unwrap();
        let plain = PlainStatement {
            commitment: p.commit(&v, Fr::from(2)).unwrap(),
            index: 0,
            value: v[0],
        };
        let proof = p.prove_plain(&plain, &data, &mut rng).unwrap();
        assert!(p.verify_plain(&plain, &proof).unwrap());
        let hidden = HiddenStatement {
            commitment: plain.commitment,
            index: 0,
            value_commitment: pedersen(v[0], Fr::from(9)),
        };
        let proof = p
            .prove_hidden(
                &hidden,
                &data,
                &HiddenWitness {
                    value: v[0],
                    randomness: Fr::from(9),
                },
                &mut rng,
            )
            .unwrap();
        assert!(p.verify_hidden(&hidden, &proof).unwrap());
    }
}
