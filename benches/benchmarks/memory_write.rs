use criterion::*;
use std::hint::black_box;
use std::time::Duration;

use super::GIB;

// Sequential writes: a memset and a memcpy over 1 GiB on one thread, and a
// memset over 16 GiB split across one pinned thread per core (16 GiB so the
// per-thread slices don't fit in L3). Buffers are touched first so page faults
// are not measured. Throughput is bytes written.
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

    let core_ids = core_affinity::get_core_ids().unwrap();
    let all_cores = 16 * GIB;
    let mut big = vec![1u8; all_cores];
    let chunk = all_cores.div_ceil(core_ids.len());
    group.throughput(Throughput::Bytes(all_cores as u64));
    group.bench_function(format!("memset, {} threads", core_ids.len()), |b| {
        b.iter(|| {
            std::thread::scope(|scope| {
                for (slice, core) in big.chunks_mut(chunk).zip(core_ids.iter().copied()) {
                    scope.spawn(move || {
                        core_affinity::set_for_current(core);
                        unsafe { std::ptr::write_bytes(slice.as_mut_ptr(), black_box(3), slice.len()) };
                        black_box(slice.as_ptr());
                    });
                }
            });
        })
    });
    group.finish();
}

criterion_group!(benches, memory_write_benchmark);
