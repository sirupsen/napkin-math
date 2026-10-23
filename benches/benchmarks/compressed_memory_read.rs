use std::time::Duration;

use bitpacking::{BitPacker, BitPacker8x};
use criterion::*;
use std::hint::black_box;

use super::MIB;

fn compressed_memory_read_benchmark(c: &mut Criterion) {
    let size_bytes = MIB;
    let bytes_per_iteration = 4; // u32
    let size_in_elements = (size_bytes as u32 / bytes_per_iteration) as u32;

    let mut data = vec![0u32; size_in_elements as usize];
    // This is faster if we set everything to e.g. 1
    // But it's unrealistic in practise.
    for i in 0..256 {
        data[i as usize] = i;
    }

    let block_len = BitPacker8x::BLOCK_LEN;
    let vector = &data[0..block_len];
    let bitpacker = BitPacker8x::new();

    let num_bits: u8 = bitpacker.num_bits(vector);
    let mut compressed = vec![vec![0u8; 256]; 1_000_000];
    let mut compressed_len = 0;
    for i in 0..1_000_000 {
        compressed_len = bitpacker.compress(vector, &mut compressed[i as usize], num_bits);
    }

    // Named so that a `memory_read` filter doesn't also select this group.
    let mut group = c.benchmark_group("bitpacking");
    let mut decompressed = vec![0; block_len];
    let mut i = 0;
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(10));
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Bytes(block_len as u64 * 4));
    group.bench_function("unpack 8x", |b| {
        b.iter(|| {
            bitpacker.decompress(
                &compressed[i][..compressed_len],
                &mut decompressed,
                num_bits,
            );
            black_box(&mut decompressed);
            if i == 1_000_000 - 1 {
                i = 0;
            } else {
                i += 1;
            }
        });
    });
    group.finish()
}

criterion_group!(benches, compressed_memory_read_benchmark);
