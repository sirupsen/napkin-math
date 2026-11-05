use criterion::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use sha2::{Digest, Sha256};
use std::hint::black_box;
use std::time::Duration;
use xxhash_rust::xxh3::xxh3_64;

use super::{KIB, MIB};

const SIZES: [usize; 13] = [
    8,
    16,
    32,
    64,
    128,
    256,
    512,
    KIB,
    4 * KIB,
    16 * KIB,
    64 * KIB,
    256 * KIB,
    MIB,
];

// One call at each size. A README row's size is the largest that takes about as long as the
// smallest, and its throughput is the fastest at any size.
fn cpu_batch_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(0xba7c);
    let bytes: Vec<u8> = (0..MIB).map(|_| rng.random()).collect();
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &[7u8; 32]).unwrap());

    let mut group = c.benchmark_group("cpu_batch");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(1));
    group.sample_size(20);
    for size in SIZES {
        group.throughput(Throughput::Bytes(size as u64));
        let input = &bytes[..size];
        group.bench_with_input(BenchmarkId::new("xxh3", size), input, |b, i| {
            b.iter(|| black_box(xxh3_64(black_box(i))))
        });
        group.bench_with_input(BenchmarkId::new("sha256", size), input, |b, i| {
            b.iter(|| black_box(Sha256::digest(black_box(i))))
        });

        let mut buffer = input.to_vec();
        let mut counter = 0u64;
        group.bench_function(BenchmarkId::new("aes-256-gcm encrypt", size), |b| {
            b.iter(|| {
                counter += 1;
                let mut nonce = [0u8; 12];
                nonce[4..].copy_from_slice(&counter.to_le_bytes());
                let tag = key
                    .seal_in_place_separate_tag(
                        Nonce::assume_unique_for_key(nonce),
                        Aad::empty(),
                        &mut buffer,
                    )
                    .unwrap();
                let _ = black_box(tag);
            })
        });

        let mut sealed = input.to_vec();
        let nonce = [1u8; 12];
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::empty(),
            &mut sealed,
        )
        .unwrap();
        let mut opened = sealed.clone();
        group.bench_function(BenchmarkId::new("aes-256-gcm decrypt", size), |b| {
            b.iter(|| {
                // Restore the ciphertext, which counts against decrypt at every size.
                opened.copy_from_slice(&sealed);
                let plain = key
                    .open_in_place(
                        Nonce::assume_unique_for_key(nonce),
                        Aad::empty(),
                        &mut opened,
                    )
                    .unwrap();
                black_box(plain.as_ptr());
            })
        });
    }
    group.finish();
}

criterion_group!(benches, cpu_batch_benchmark);
