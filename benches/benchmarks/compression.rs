use criterion::*;
use std::hint::black_box;

use super::workload::compression_corpus;

fn compression_benchmark(c: &mut Criterion) {
    let corpus = compression_corpus();
    let bytes = corpus.bytes;
    let compressed = corpus.compressed;
    let level = corpus.level;

    let mut group = c.benchmark_group("compression");
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function(format!("zstd/level {level}"), |b| {
        b.iter(|| black_box(zstd::bulk::compress(&bytes, level).unwrap()))
    });
    group.finish();

    let mut group = c.benchmark_group("decompression");
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function(format!("zstd/level {level}"), |b| {
        b.iter(|| black_box(zstd::bulk::decompress(&compressed, bytes.len()).unwrap()))
    });
    group.finish();
}

criterion_group!(benches, compression_benchmark);
