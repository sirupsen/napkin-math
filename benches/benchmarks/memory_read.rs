use criterion::*;

#[cfg(target_arch = "x86_64")]
use std::arch::{asm, x86_64::*};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

type Int = u64;

struct ThreadedMemoryReadBenchmark {
    start: Arc<Barrier>,
    end: Arc<Barrier>,
    total: Arc<AtomicU64>,
}

impl ThreadedMemoryReadBenchmark {
    fn new(
        core_ids: Arc<Vec<core_affinity::CoreId>>,
        size_in_elements: usize,
        f: fn(&[Int]) -> u64,
    ) -> Self {
        let num_cores = core_ids.len();
        let slice_size = size_in_elements / num_cores;

        let start = Arc::new(Barrier::new(num_cores + 1));
        let end = Arc::new(Barrier::new(num_cores + 1));
        let done_allocating = Arc::new(Barrier::new(num_cores + 1));
        let total = Arc::new(AtomicU64::new(0));

        for (index, core) in core_ids.iter().copied().enumerate() {
            let t_start = start.clone();
            let t_end = end.clone();
            let t_total = total.clone();
            let t_done_allocating = done_allocating.clone();

            thread::spawn(move || {
                // Ensure each thread is scheduled on a different core to maximize memory bandwidth.
                // Each core can typically only have ~10 outstanding requests to the L1 cache.
                // https://news.ycombinator.com/item?id=16174813
                core_affinity::set_for_current(core);

                let range_start = slice_size * index;
                let range_end = if index + 1 == num_cores {
                    size_in_elements
                } else {
                    range_start + slice_size
                };
                let mut vec = Vec::with_capacity(range_end - range_start);
                for i in range_start..range_end {
                    vec.push(i as Int);
                }

                t_done_allocating.wait();
                loop {
                    t_start.wait();
                    t_total.fetch_add(f(&vec), Ordering::Relaxed);
                    t_end.wait();
                }
            });
        }

        done_allocating.wait();

        Self { start, end, total }
    }

    fn run(&self) -> u64 {
        self.total.store(0, Ordering::Relaxed);
        self.start.wait();
        self.end.wait();
        self.total.load(Ordering::Relaxed)
    }
}

#[inline(never)]
#[no_mangle]
fn memory_read_sequential_single_thread_vectorized(vec: &[Int]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                return memory_read_sequential_single_thread_vectorized_avx2(vec);
            }
        }
    }

    let mut sum = 0u64;
    for value in vec {
        sum = sum.wrapping_add(*value);
    }

    sum
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn memory_read_sequential_single_thread_vectorized_avx2(vec: &[Int]) -> u64 {
    let mut acc0 = _mm256_setzero_si256();
    let mut acc1 = _mm256_setzero_si256();
    let mut acc2 = _mm256_setzero_si256();
    let mut acc3 = _mm256_setzero_si256();
    let mut i = 0usize;
    let ptr = vec.as_ptr();

    while i + 16 <= vec.len() {
        acc0 = _mm256_add_epi64(acc0, _mm256_loadu_si256(ptr.add(i) as *const __m256i));
        acc1 = _mm256_add_epi64(acc1, _mm256_loadu_si256(ptr.add(i + 4) as *const __m256i));
        acc2 = _mm256_add_epi64(acc2, _mm256_loadu_si256(ptr.add(i + 8) as *const __m256i));
        acc3 = _mm256_add_epi64(acc3, _mm256_loadu_si256(ptr.add(i + 12) as *const __m256i));
        i += 16;
    }

    acc0 = _mm256_add_epi64(acc0, acc1);
    acc2 = _mm256_add_epi64(acc2, acc3);
    acc0 = _mm256_add_epi64(acc0, acc2);

    let mut lanes = [0u64; 4];
    unsafe {
        _mm256_storeu_si256(lanes.as_mut_ptr() as *mut __m256i, acc0);
    }

    let mut sum = lanes.iter().copied().sum::<u64>();
    while i < vec.len() {
        sum = sum.wrapping_add(unsafe { *ptr.add(i) });
        i += 1;
    }

    sum
}

#[inline(never)]
#[no_mangle]
fn memory_read_sequential_single_thread_non_vectorized(vec: &[Int]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        unsafe {
            return memory_read_sequential_single_thread_non_vectorized_x86(vec);
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        let mut sum = 0u64;
        let ptr = vec.as_ptr();

        for i in 0..vec.len() {
            // Volatile scalar loads keep this benchmark on a real non-SIMD path.
            sum = sum.wrapping_add(unsafe { std::ptr::read_volatile(ptr.add(i)) });
        }

        sum
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(never)]
unsafe fn memory_read_sequential_single_thread_non_vectorized_x86(vec: &[Int]) -> u64 {
    let ptr = vec.as_ptr();
    let remaining = vec.len();
    let sum: u64;

    asm!(
        "xor {sum}, {sum}",
        "test {remaining}, {remaining}",
        "je 3f",
        "2:",
        "mov {tmp}, qword ptr [{ptr}]",
        "add {sum}, {tmp}",
        "add {ptr}, 8",
        "dec {remaining}",
        "jne 2b",
        "3:",
        ptr = inout(reg) ptr => _,
        remaining = inout(reg) remaining => _,
        sum = lateout(reg) sum,
        tmp = lateout(reg) _,
        options(nostack, readonly),
    );

    sum
}

fn memory_read_benchmark(c: &mut Criterion) {
    let bytes_per_iteration = 8usize;
    let size_bytes = n_gib_bytes!(1) as usize;
    let size_in_elements = size_bytes / bytes_per_iteration;
    let vec: Vec<Int> = (0..size_in_elements).map(|i| i as Int).collect();

    // Keep all-core benchmarks on a single hardware thread per core after `./run` disables HT.
    let core_ids = Arc::new(core_affinity::get_core_ids().unwrap());
    let threaded_non_vectorized = ThreadedMemoryReadBenchmark::new(
        core_ids.clone(),
        size_in_elements,
        memory_read_sequential_single_thread_non_vectorized,
    );
    let threaded_vectorized = ThreadedMemoryReadBenchmark::new(
        core_ids.clone(),
        size_in_elements,
        memory_read_sequential_single_thread_vectorized,
    );

    let mut group = c.benchmark_group("memory_read");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(10));
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Bytes(size_bytes as u64));
    group.bench_function("1 thread, No SIMD", |b| {
        b.iter(|| black_box(memory_read_sequential_single_thread_non_vectorized(&vec)))
    });
    group.bench_function("1 thread, SIMD", |b| {
        b.iter(|| black_box(memory_read_sequential_single_thread_vectorized(&vec)))
    });
    group.bench_function(format!("{} threads, No SIMD", core_ids.len()), |b| {
        b.iter(|| {
            black_box(threaded_non_vectorized.run());
        })
    });
    group.bench_function(format!("{} threads, SIMD", core_ids.len()), |b| {
        b.iter(|| {
            black_box(threaded_vectorized.run());
        })
    });
    group.finish()
}

criterion_group!(benches, memory_read_benchmark);
