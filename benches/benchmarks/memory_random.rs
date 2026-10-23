use criterion::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::mem::forget;
use std::ptr;
use std::time::Instant;

type CacheLine = [u64; 8];

struct RandomMemoryTest {
    vec: Vec<CacheLine>,
    order: Vec<usize>,
    i: usize,
}

fn random_memory_write_setup() -> RandomMemoryTest {
    let size_in_elements = (n_gb_bytes!(1) as u64 / 64) as usize;

    let mut vec = Vec::new();
    vec.resize(size_in_elements, [1, 2, 3, 4, 5, 6, 7, 8]);

    let mut order: Vec<usize> = (0..size_in_elements).collect();
    order.shuffle(&mut thread_rng());

    RandomMemoryTest { vec, order, i: 0 }
}

fn random_memory_read_setup() -> RandomMemoryTest {
    let size_in_elements = (n_gb_bytes!(1) as u64 / 64) as usize;

    let mut vec = Vec::new();
    vec.resize(size_in_elements, [1, 2, 3, 4, 5, 6, 7, 8]);
    unsafe {
        let data = vec.as_mut_ptr() as *mut libc::c_void;
        libc::madvise(
            data,
            size_in_elements * std::mem::size_of::<CacheLine>(),
            libc::MADV_RANDOM,
        );
    }

    let mut order: Vec<usize> = (0..size_in_elements).collect();
    unsafe {
        let data = order.as_mut_ptr() as *mut libc::c_void;
        libc::madvise(
            data,
            size_in_elements * std::mem::size_of::<usize>(),
            libc::MADV_SEQUENTIAL,
        );
    }
    order.shuffle(&mut thread_rng());

    RandomMemoryTest { vec, order, i: 0 }
}

fn legacy_black_box<T>(dummy: T) -> T {
    unsafe {
        let ret = ptr::read_volatile(&dummy);
        forget(dummy);
        ret
    }
}

fn random_memory_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("random_memory");
    group.sample_size(10);
    group.throughput(Throughput::Bytes(n_gb_bytes!(1) as u64));

    {
        group.bench_function("read", |b| {
            b.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;
                for _ in 0..iters {
                    let mut test = random_memory_read_setup();
                    let start = Instant::now();
                    loop {
                        legacy_black_box(test.vec[test.order[test.i]]);
                        test.i += 1;
                        if test.i == test.vec.len() {
                            break;
                        }
                    }
                    total += start.elapsed();
                }
                total
            })
        });
    }

    {
        group.bench_function("write", |b| {
            b.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;
                for _ in 0..iters {
                    let mut test = random_memory_write_setup();
                    let start = Instant::now();
                    loop {
                        test.vec[test.order[test.i]] = [8, 7, 6, 5, 4, 3, 2, 1];
                        legacy_black_box(test.vec[test.order[test.i]]);
                        test.i += 1;
                        if test.i == test.vec.len() {
                            break;
                        }
                    }
                    total += start.elapsed();
                }
                total
            })
        });
    }

    group.finish();
}

criterion_group!(benches, random_memory_benchmark);
