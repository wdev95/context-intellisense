//! Benchmark over the hand-written corpus in `corpus/`: how many constructions convert in each
//! mode, and how long a conversion takes.
//!
//! `cargo run --release -p contextEquation --example bench`

use std::time::Instant;

use context_equation_core::{convert, Options};

/// Conversions of each equation timed per sample; the time of a sample is their mean.
const REPEAT: u32 = 200;

fn load(name: &str) -> Vec<(String, String)> {
    let path = format!("{}/../../corpus/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn main() {
    let mut corpus = load("constructions.json");
    corpus.extend(load("packages.json"));

    let strict = Options { strict: true, ..Options::default() };
    let tolerant = Options::default();
    let (mut strict_ok, mut clean, mut warned, mut failed) = (0, 0, 0, 0);
    let mut times = Vec::with_capacity(corpus.len());

    for (_, tex) in &corpus {
        if convert(tex, &strict).is_ok() {
            strict_ok += 1;
        }
        match convert(tex, &tolerant) {
            Ok(r) if r.warnings.is_empty() => clean += 1,
            Ok(_) => warned += 1,
            Err(_) => failed += 1,
        }
        let start = Instant::now();
        for _ in 0..REPEAT {
            let _ = std::hint::black_box(convert(std::hint::black_box(tex), &tolerant));
        }
        times.push(start.elapsed().as_secs_f64() * 1e6 / f64::from(REPEAT));
    }

    times.sort_by(f64::total_cmp);
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let p95 = times[((times.len() as f64 * 0.95).ceil() as usize).min(times.len()) - 1];
    let total = corpus.len();
    let pct = |n: usize| 100.0 * n as f64 / total as f64;

    println!("| Measure | Value |");
    println!("|---|---|");
    println!("| Constructions | {total} |");
    println!("| Strict: converted | {strict_ok} ({:.1}%) |", pct(strict_ok));
    println!("| Tolerant: no warning | {clean} ({:.1}%) |", pct(clean));
    println!("| Tolerant: with warnings | {warned} ({:.1}%) |", pct(warned));
    println!("| Tolerant: error | {failed} ({:.1}%) |", pct(failed));
    println!("| Time per equation, mean | {mean:.1} µs |");
    println!("| Time per equation, p95 | {p95:.1} µs |");
}
