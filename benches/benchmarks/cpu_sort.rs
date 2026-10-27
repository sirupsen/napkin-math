use criterion::*;
use std::hint::black_box;

use super::{GIB, MIB};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

fn cpu_sort_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(0x1337);

    let mut group = c.benchmark_group("cpu_sort");
    group.sample_size(10);
    group.sampling_mode(SamplingMode::Flat);

    // 1 MiB fits in L2, so it is the best case. The README's 1 GiB column is
    // what people extrapolate to, so measure that directly instead of assuming
    // sorting is linear.
    let sizes = [
        ("u64/1 MiB", MIB),
        ("u64/64 MiB", 64 * MIB),
        ("u64/1 GiB", GIB),
    ];

    for (name, total_size) in sizes.iter() {
        let elements = total_size / std::mem::size_of::<u64>();
        let input: Vec<u64> = (0..elements).map(|_| rng.random()).collect();

        group.throughput(Throughput::Bytes(*total_size as u64));
        group.bench_function(*name, |b| {
            b.iter_batched(
                || input.clone(),
                |mut values| {
                    values.sort_unstable();
                    black_box(values[0])
                },
                BatchSize::LargeInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, cpu_sort_benchmark);
