// Timing helpers shared by the examples. Include with
// `#[path = "common/timing.rs"] mod timing;`.
#![allow(dead_code)]
use std::time::{Duration, Instant};

/// Seconds taken by the fastest of `runs` calls.
pub fn best_of(runs: usize, mut f: impl FnMut()) -> f64 {
    (0..runs).map(|_| time(&mut f)).fold(f64::MAX, f64::min)
}

/// Seconds taken by the fastest call, calling `f` again until `min` has passed.
pub fn best_for(min: Duration, mut f: impl FnMut()) -> f64 {
    let started = Instant::now();
    let mut best = f64::MAX;
    while best == f64::MAX || started.elapsed() < min {
        best = best.min(time(&mut f));
    }
    best
}

fn time(f: &mut impl FnMut()) -> f64 {
    let start = Instant::now();
    f();
    start.elapsed().as_secs_f64()
}

pub fn gib_s(bytes: usize, secs: f64) -> f64 {
    bytes as f64 / secs / (1u64 << 30) as f64
}

pub fn mib_s(bytes: usize, secs: f64) -> f64 {
    bytes as f64 / secs / (1u64 << 20) as f64
}
