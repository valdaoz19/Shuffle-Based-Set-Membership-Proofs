use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct Stats {
    pub count: usize,
    pub median: f64,
    pub q1: f64,
    pub q3: f64,
    pub iqr: f64,
    pub mean: f64,
    pub sample_stddev: Option<f64>,
    pub min: f64,
    pub max: f64,
    pub mean_ci95: [f64; 2],
    pub bootstrap_resamples: usize,
    pub bootstrap_seed: u64,
}
fn quantile(sorted: &[f64], p: f64) -> f64 {
    let i = p * (sorted.len() - 1) as f64;
    let lo = i.floor() as usize;
    sorted[lo] + (sorted[i.ceil() as usize] - sorted[lo]) * (i - lo as f64)
}
pub fn summarize(values: &[f64], seed: u64) -> Option<Stats> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let count = v.len();
    let mean = v.iter().sum::<f64>() / count as f64;
    let sample_stddev = if count > 1 {
        Some((v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (count - 1) as f64).sqrt())
    } else {
        None
    };
    let mut rng = StdRng::seed_from_u64(seed);
    let mut samples = Vec::with_capacity(10000);
    for _ in 0..10000 {
        samples.push((0..count).map(|_| v[rng.gen_range(0..count)]).sum::<f64>() / count as f64);
    }
    samples.sort_by(f64::total_cmp);
    let q1 = quantile(&v, 0.25);
    let q3 = quantile(&v, 0.75);
    Some(Stats {
        count,
        median: quantile(&v, 0.5),
        q1,
        q3,
        iqr: q3 - q1,
        mean,
        sample_stddev,
        min: v[0],
        max: v[count - 1],
        mean_ci95: [quantile(&samples, 0.025), quantile(&samples, 0.975)],
        bootstrap_resamples: 10000,
        bootstrap_seed: seed,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic() {
        let s = summarize(&[1., 2., 3., 4.], 9).unwrap();
        assert_eq!(s.mean, 2.5);
        assert_eq!(s.median, 2.5);
        assert_eq!(s.iqr, 1.5);
        assert!((s.sample_stddev.unwrap() - (5f64 / 3.).sqrt()).abs() < 1e-12);
        assert!(summarize(&[], 9).is_none());
        assert!(summarize(&[1.], 9).unwrap().sample_stddev.is_none());
        assert_eq!(
            s.mean_ci95,
            summarize(&[1., 2., 3., 4.], 9).unwrap().mean_ci95
        );
    }
}
