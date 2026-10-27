use criterion::*;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, CHACHA20_POLY1305};
use std::hint::black_box;

use super::KIB;

// Authenticated encryption and decryption of one 16 KiB buffer (a full TLS
// record) with ring, the same code rustls uses.
fn cpu_encrypt_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_encrypt");
    group.throughput(Throughput::Bytes(16 * KIB as u64));

    for (name, algorithm) in [
        ("aes-256-gcm", &AES_256_GCM),
        ("chacha20-poly1305", &CHACHA20_POLY1305),
    ] {
        let key = LessSafeKey::new(UnboundKey::new(algorithm, &[7u8; 32]).unwrap());
        let mut buffer = vec![0u8; 16 * KIB];
        let mut counter = 0u64;
        group.bench_function(name, |b| {
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

        let mut sealed = vec![0u8; 16 * KIB];
        let nonce = [1u8; 12];
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::empty(),
            &mut sealed,
        )
        .unwrap();
        let mut opened = sealed.clone();
        group.bench_function(format!("{name} decrypt"), |b| {
            b.iter(|| {
                // Restore the ciphertext: a 16 KiB copy within L1, a few percent of the time.
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

criterion_group!(benches, cpu_encrypt_benchmark);
