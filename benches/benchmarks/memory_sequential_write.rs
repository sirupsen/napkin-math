use criterion::*;
use std::hint::black_box;
use std::time::Duration;

use super::GIB;

// Writes that bypass the cache. A regular store first reads the line it
// overwrites (read for ownership), so an x86 memset moves each byte twice and
// runs at ~11 GiB/s on one thread; non-temporal stores skip that read and reach
// ~24. Graviton4 skips the read on its own for full-line writes, so ARM
// doesn't need this.
#[cfg(target_arch = "x86_64")]
fn memset_non_temporal(dst: &mut [u8]) {
    use std::arch::x86_64::*;
    #[target_feature(enable = "avx512f")]
    unsafe fn avx512(p: *mut u8, n: usize) {
        let v = _mm512_set1_epi8(3);
        let mut i = 0;
        while i + 256 <= n {
            for j in 0..4 {
                _mm512_stream_si512(p.add(i + j * 64).cast(), v);
            }
            i += 256;
        }
        _mm_sfence();
    }
    #[target_feature(enable = "avx2")]
    unsafe fn avx2(p: *mut u8, n: usize) {
        let v = _mm256_set1_epi8(3);
        let mut i = 0;
        while i + 128 <= n {
            for j in 0..4 {
                _mm256_stream_si256(p.add(i + j * 32).cast(), v);
            }
            i += 128;
        }
        _mm_sfence();
    }
    // Buffers come from page-aligned allocations; the slices below are 4 KiB multiples.
    assert!((dst.as_ptr() as usize).is_multiple_of(64) && dst.len().is_multiple_of(256));
    unsafe {
        if is_x86_feature_detected!("avx512f") {
            avx512(dst.as_mut_ptr(), dst.len())
        } else {
            avx2(dst.as_mut_ptr(), dst.len())
        }
    }
}

// Sequential writes: a memset and a memcpy over 1 GiB on one thread, and a
// memset over 16 GiB split across one pinned thread per core (16 GiB so the
// per-thread slices don't fit in L3), plus non-temporal memsets on x86. Buffers
// are touched first so page faults are not measured. Throughput is bytes written.
fn memory_sequential_write_benchmark(c: &mut Criterion) {
    let mut dst = vec![1u8; GIB];
    let src = vec![2u8; GIB];

    let mut group = c.benchmark_group("memory_sequential_write");
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
    #[cfg(target_arch = "x86_64")]
    group.bench_function("memset, non-temporal", |b| {
        b.iter(|| {
            memset_non_temporal(&mut dst);
            black_box(dst.as_ptr());
        })
    });

    let core_ids = core_affinity::get_core_ids().unwrap();
    let all_cores = 16 * GIB;
    let mut big = vec![1u8; all_cores];
    let chunk = (all_cores / core_ids.len()) & !4095;
    group.throughput(Throughput::Bytes(all_cores as u64));
    group.bench_function(format!("memset, {} threads", core_ids.len()), |b| {
        b.iter(|| {
            std::thread::scope(|scope| {
                for (slice, core) in big.chunks_mut(chunk).zip(core_ids.iter().copied()) {
                    scope.spawn(move || {
                        core_affinity::set_for_current(core);
                        unsafe {
                            std::ptr::write_bytes(slice.as_mut_ptr(), black_box(3), slice.len())
                        };
                        black_box(slice.as_ptr());
                    });
                }
            });
        })
    });
    #[cfg(target_arch = "x86_64")]
    group.bench_function(format!("memset, non-temporal, {} threads", core_ids.len()), |b| {
        b.iter(|| {
            std::thread::scope(|scope| {
                for (slice, core) in big.chunks_mut(chunk).zip(core_ids.iter().copied()) {
                    scope.spawn(move || {
                        core_affinity::set_for_current(core);
                        if slice.len() == chunk {
                            memset_non_temporal(slice);
                        }
                        black_box(slice.as_ptr());
                    });
                }
            });
        })
    });
    group.finish();
}

criterion_group!(benches, memory_sequential_write_benchmark);
