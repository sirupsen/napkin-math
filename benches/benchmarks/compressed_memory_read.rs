use std::time::{Duration, Instant};

use bitpacking::{BitPacker, BitPacker1x, BitPacker4x, BitPacker8x};
use criterion::*;

fn compressed_memory_read_benchmark(c: &mut Criterion) {
    let size_bytes = n_mib_bytes!(1);
    let bytes_per_iteration = 4; // u32
    let size_in_elements = (size_bytes as u32 / bytes_per_iteration) as u32;

    let mut data = vec![0u32; size_in_elements as usize];
    // This is faster if we set everything to e.g. 1
    // But it's unrealistic in practise.
    for i in 0..256 {
        data[i as usize] = i;
    }

    if is_x86_feature_detected!("sse3") {
        println!("SSE3 detected");
    }

    if is_x86_feature_detected!("avx2") {
        println!("AVX2 detected");
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

    // println!("vector length: {}", vector.len());
    // println!("vector: {:?}", vector);
    // println!("compressed: {:?}", compressed);
    println!(
        "compressed_len: {}, length of vec: {}, bits: {}",
        compressed_len,
        4 * block_len,
        num_bits,
    );
    println!("block_length: {}", block_len);

    let mut group = c.benchmark_group("compressed_memory_read");
    let mut decompressed = vec![0; block_len];
    let mut i = 0;
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(10));
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Bytes(block_len as u64 * 4));
    // group.throughput(Throughput::Elements(block_len as u64));
    group.bench_function("bitpacker", |b| {
        b.iter(|| {
            bitpacker.decompress(
                &compressed[i][..compressed_len],
                &mut decompressed,
                num_bits,
            );
            if i == 1_000_000 - 1 {
                i = 0;
            } else {
                i += 1;
            }
        });
    });
    group.finish()
}

// #[inline(never)]
// #[no_mangle]
// fn simon_do_stuff(
//     bitpacker: &BitPacker8x,
//     compressed: &Vec<u8>,
//     compressed_len: usize,
//     decompressed: &mut Vec<Vec<u32>>,
//     offset: &mut u8,
//     num_bits: u8,
// ) {
//     bitpacker.decompress(
//         &compressed[..compressed_len],
//         &mut decompressed[*offset as usize],
//         num_bits,
//     );
//     *offset = offset.wrapping_add(1);
// }

criterion_group!(benches, compressed_memory_read_benchmark);
