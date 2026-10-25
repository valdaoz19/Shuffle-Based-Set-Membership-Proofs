use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rand::rngs::OsRng;

use aml_crypto_ibc::{IbcScheme, IdEqProver, IdEqVerifier, PublicParams, VVerProver, VVerVerifier};
use aml_crypto_shuffle::VerifiableShuffle;

fn bench_ibc(c: &mut Criterion) {
    let pp = PublicParams::setup();
    let mut rng = OsRng;

    c.bench_function("ibc_commit", |b| {
        b.iter(|| {
            let (_commit, _open) = IbcScheme::commit(black_box(&pp), 42, 10_000, &mut rng);
        });
    });

    let (commit, open) = IbcScheme::commit(&pp, 42, 10_000, &mut rng);
    c.bench_function("ibc_verify", |b| {
        b.iter(|| {
            black_box(IbcScheme::verify(
                black_box(&pp),
                black_box(&commit),
                black_box(&open),
            ));
        });
    });

    let (commit2, open2) = IbcScheme::commit(&pp, 42, 20_000, &mut rng);
    let id_eq_proof = IdEqProver::prove(&pp, &open, &open2, &commit, &commit2, &mut rng);
    c.bench_function("ibc_ideq_verify", |b| {
        b.iter(|| {
            black_box(IdEqVerifier::verify(
                black_box(&pp),
                black_box(&commit),
                black_box(&commit2),
                black_box(&id_eq_proof),
            ));
        });
    });

    let vver = VVerProver::prove(&pp, &open, &commit, 10_000, &mut rng);
    c.bench_function("ibc_vver_verify", |b| {
        b.iter(|| {
            black_box(VVerVerifier::verify(
                black_box(&pp),
                black_box(&commit),
                10_000,
                black_box(&vver),
            ));
        });
    });
}

fn bench_shuffle(c: &mut Criterion) {
    let pp = PublicParams::setup();
    let mut rng = OsRng;
    let n = 100usize;
    let mut inputs = Vec::with_capacity(n);
    let mut openings = Vec::with_capacity(n);
    for i in 0..n {
        let (commit, open) = IbcScheme::commit(&pp, 1, i as u64 * 10, &mut rng);
        inputs.push(commit);
        openings.push(open);
    }

    c.bench_function("shuffle_prove_n100", |b| {
        b.iter(|| {
            let (proof, _openings) = VerifiableShuffle::shuffle_and_prove(
                black_box(&pp),
                black_box(&inputs),
                black_box(&openings),
                &mut rng,
            );
            black_box(proof);
        });
    });

    let (proof, _) = VerifiableShuffle::shuffle_and_prove(&pp, &inputs, &openings, &mut rng);
    c.bench_function("shuffle_verify_n100", |b| {
        b.iter(|| {
            black_box(VerifiableShuffle::verify(
                black_box(&pp),
                black_box(&inputs),
                black_box(&proof),
            ));
        });
    });
}

criterion_group!(benches, bench_ibc, bench_shuffle);
criterion_main!(benches);
