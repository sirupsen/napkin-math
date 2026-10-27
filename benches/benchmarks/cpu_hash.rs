use crc32fast::Hasher as Crc32Hasher;
use criterion::*;
use std::hint::black_box;

use super::MIB;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use sha2::{Digest, Sha256};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use xxhash_rust::xxh3::xxh3_64;

// Inputs go through black_box too: the hashes are pure functions of a buffer that never changes,
// so otherwise the compiler may hash once and hoist the result out of the loop.
fn hash_group(c: &mut Criterion, name: &str, bytes: &[u8]) {
    let mut group = c.benchmark_group(name);
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function("sha256", |b| {
        b.iter(|| black_box(Sha256::digest(black_box(bytes))))
    });
    group.bench_function("blake3", |b| {
        b.iter(|| black_box(blake3::hash(black_box(bytes))))
    });
    group.bench_function("crc32", |b| {
        b.iter(|| {
            let mut hasher = Crc32Hasher::new();
            hasher.update(black_box(bytes));
            black_box(hasher.finalize())
        })
    });
    group.bench_function("siphash", |b| {
        b.iter(|| {
            let mut hasher = DefaultHasher::new();
            hasher.write(black_box(bytes));
            black_box(hasher.finish())
        })
    });
    group.bench_function("xxh3", |b| b.iter(|| black_box(xxh3_64(black_box(bytes)))));
    group.finish();
}

fn cpu_hash_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(0xfeed_face);
    let bytes: Vec<u8> = (0..MIB).map(|_| rng.random()).collect();

    // The README rows are per 64-byte call, where per-call setup dominates.
    hash_group(c, "cpu_hash", &bytes[..64]);
    // Bulk throughput over a buffer that still fits in L2/L3.
    hash_group(c, "cpu_hash_1mib", &bytes);
}

criterion_group!(benches, cpu_hash_benchmark);
