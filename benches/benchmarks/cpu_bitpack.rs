use bitpacking::{BitPacker, BitPacker4x};
use criterion::*;
use std::hint::black_box;

use super::{any_selected, GIB, MIB};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

const SIZE: usize = 256 * MIB;

// 256 MiB of u32s at 12 bits each, packed and unpacked in Tantivy's 128-value
// blocks, so the packed side (96 MiB) streams from memory as a column would.
// Unpack decodes each block into the same buffer, like a reader consuming a
// column. Throughput is against the u32s.
fn cpu_bitpack_benchmark(c: &mut Criterion) {
    if !any_selected("cpu_bitpack", ["pack, 12 bits", "unpack, 12 bits"]) {
        return;
    }
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

// Every core unpacks its own slice of 6 GiB of packed 12-bit integers (16 GiB as u32s), far past
// any cache, into its own 128-value buffer. One core is close to memory-bound already (it reads
// ~20 GiB/s of packed input), so all cores show how far memory bandwidth times the compression
// ratio goes. Throughput is against the u32s, like the single-thread rows. Unpacking doesn't
// branch on the values, so the input is a cheap pseudo-random pattern written by the thread that
// reads it.
fn cpu_bitpack_all_cores_benchmark(c: &mut Criterion) {
    const BITS: u8 = 12;
    let packer = BitPacker4x::new();
    let block_bytes = BitPacker4x::BLOCK_LEN * BITS as usize / 8;
    let cores = core_affinity::get_core_ids().unwrap();
    let name = format!("unpack, 12 bits, {} threads", cores.len());
    if !any_selected("cpu_bitpack_all_cores", [&name]) {
        return;
    }
    let blocks = 16 * GIB / 4 / BitPacker4x::BLOCK_LEN;
    let per_core = blocks / cores.len();
    let mut packed = vec![0u8; per_core * cores.len() * block_bytes];
    std::thread::scope(|s| {
        for (core, chunk) in cores.iter().zip(packed.chunks_mut(per_core * block_bytes)) {
            s.spawn(move || {
                core_affinity::set_for_current(*core);
                let mut x = 0x9e37_79b9_7f4a_7c15u64 ^ core.id as u64;
                for word in chunk.chunks_mut(8) {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    word.copy_from_slice(&x.to_le_bytes()[..word.len()]);
                }
            });
        }
    });

    let mut group = c.benchmark_group("cpu_bitpack_all_cores");
    group.sample_size(10);
    group.throughput(Throughput::Bytes(
        (packed.len() / block_bytes * BitPacker4x::BLOCK_LEN * 4) as u64,
    ));
    group.bench_function(name, |b| {
        b.iter(|| {
            std::thread::scope(|s| {
                for (core, chunk) in cores.iter().zip(packed.chunks(per_core * block_bytes)) {
                    s.spawn(move || {
                        core_affinity::set_for_current(*core);
                        let mut unpacked = [0u32; BitPacker4x::BLOCK_LEN];
                        for p in chunk.chunks(block_bytes) {
                            packer.decompress(black_box(p), &mut unpacked, BITS);
                            black_box(&unpacked);
                        }
                    });
                }
            })
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    cpu_bitpack_benchmark,
    cpu_bitpack_all_cores_benchmark
);
