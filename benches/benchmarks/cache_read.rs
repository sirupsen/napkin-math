use criterion::*;
use std::hint::black_box;

use super::memory_random::chase_group;
use super::memory_sequential_read::{memory_read_sequential_single_thread_vectorized, Int};
use super::{KIB, MIB};

// The same single-thread SIMD read over buffers that fit in L1, L2 and L3,
// read again and again.
fn cache_read_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_read");
    for (name, bytes) in [
        ("16 KiB", 16 * KIB),
        ("256 KiB", 256 * KIB),
        ("8 MiB", 8 * MIB),
    ] {
        let vec: Vec<Int> = (0..bytes / 8).map(|i| i as Int).collect();
        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_function(name, |b| {
            b.iter(|| {
                black_box(memory_read_sequential_single_thread_vectorized(black_box(
                    &vec,
                )))
            })
        });
    }
    group.finish()
}

// Latency of the same sizes: a dependent pointer chase that stays in L1, L2 or L3.
fn cache_read_latency_benchmark(c: &mut Criterion) {
    chase_group(
        c,
        "cache_read_latency",
        &[
            ("16 KiB", 16 * KIB, true),
            ("256 KiB", 256 * KIB, true),
            ("8 MiB", 8 * MIB, true),
        ],
    );
}

criterion_group!(benches, cache_read_benchmark, cache_read_latency_benchmark);
