use criterion::*;
use std::hint::black_box;
use std::time::Duration;

use super::GIB;

// Single-threaded sequential writes over 1 GiB: a memset, and a memcpy between
// two 1 GiB buffers. Both buffers are touched first so page faults are not
// measured. Throughput is bytes written.
fn memory_write_benchmark(c: &mut Criterion) {
    let mut dst = vec![1u8; GIB];
    let src = vec![2u8; GIB];

    let mut group = c.benchmark_group("memory_write");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Bytes(GIB as u64));
    group.bench_function("memset", |b| {
        b.iter(|| {
            unsafe { std::ptr::write_bytes(dst.as_mut_ptr(), black_box(3), dst.len()) };
            black_box(dst.as_ptr());
        })
    });
    group.bench_function("memcpy", |b| {
        b.iter(|| {
            dst.copy_from_slice(black_box(&src));
            black_box(dst.as_ptr());
        })
    });
    group.finish();
}

criterion_group!(benches, memory_write_benchmark);
