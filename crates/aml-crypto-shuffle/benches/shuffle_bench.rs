use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rand::rngs::OsRng;

use aml_crypto_ibc::{IbcScheme, PublicParams};
use aml_crypto_shuffle::VerifiableShuffle;

fn bench_shuffle(c: &mut Criterion) {
    let pp = PublicParams::setup();
    let mut rng = OsRng;
    let n = 100usize;
    let mut inputs = Vec::with_capacity(n);
    let mut openings = Vec::with_capacity(n);
    for i in 0..n {
        let (c, o) = IbcScheme::commit(&pp, 1, i as u64 * 10, &mut rng);
        inputs.push(c);
        openings.push(o);
    }

    c.bench_function("shuffle_prove_n100", |b| {
        b.iter(|| {
            black_box(VerifiableShuffle::shuffle_and_prove(
                black_box(&pp),
                black_box(&inputs),
                black_box(&openings),
                &mut rng,
            ));
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

criterion_group!(shuffle_benches, bench_shuffle);
criterion_main!(shuffle_benches);
