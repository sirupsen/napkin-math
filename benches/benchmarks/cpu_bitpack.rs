use bitpacking::{BitPacker, BitPacker4x};
use criterion::*;
use std::hint::black_box;

use super::MIB;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

const SIZE: usize = 256 * MIB;

// 256 MiB of u32s at 12 bits each, packed and unpacked in Tantivy's 128-value
// blocks, so the packed side (96 MiB) streams from memory as a column would.
// Unpack decodes each block into the same buffer, like a reader consuming a
// column. Throughput is against the u32s.
fn cpu_bitpack_benchmark(c: &mut Criterion) {
    const BITS: u8 = 12;
    let mut rng = StdRng::seed_from_u64(0xb175);
    let values: Vec<u32> = (0..SIZE / 4)
        .map(|_| rng.random::<u32>() >> (32 - BITS))
        .collect();
    let packer = BitPacker4x::new();
    let block = BitPacker4x::BLOCK_LEN;
    let block_bytes = block * BITS as usize / 8;
    let mut packed = vec![0u8; values.len() / block * block_bytes];
    let mut unpacked = vec![0u32; block];
    // Check one round trip up front: a filtered run may skip either benchmark.
    for (v, p) in values.chunks(block).zip(packed.chunks_mut(block_bytes)) {
        packer.compress(v, p, BITS);
    }
    packer.decompress(&packed[packed.len() - block_bytes..], &mut unpacked, BITS);
    assert_eq!(unpacked, values[values.len() - block..]);

    let mut group = c.benchmark_group("cpu_bitpack");
    group.sample_size(20);
    group.throughput(Throughput::Bytes(SIZE as u64));
    group.bench_function("pack, 12 bits", |b| {
        b.iter(|| {
            for (v, p) in values.chunks(block).zip(packed.chunks_mut(block_bytes)) {
                packer.compress(black_box(v), p, BITS);
            }
        })
    });
    group.bench_function("unpack, 12 bits", |b| {
        b.iter(|| {
            for p in packed.chunks(block_bytes) {
                packer.decompress(black_box(p), &mut unpacked, BITS);
                black_box(&unpacked);
            }
        })
    });
    group.finish();
}

criterion_group!(benches, cpu_bitpack_benchmark);
