use criterion::*;

use super::GIB;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};
use std::hint::black_box;
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
    let size_in_elements = (GIB as u64 / 64) as usize;

    let mut vec = Vec::new();
    vec.resize(size_in_elements, [1, 2, 3, 4, 5, 6, 7, 8]);

    let mut order: Vec<usize> = (0..size_in_elements).collect();
    order.shuffle(&mut rand::rng());

    RandomMemoryTest { vec, order, i: 0 }
}

fn random_memory_read_setup(huge_pages: bool) -> RandomMemoryTest {
    let size_in_elements = (GIB as u64 / 64) as usize;

    // Advise before the first touch, or the pages are already 4 KiB.
    let mut vec: Vec<CacheLine> = Vec::with_capacity(size_in_elements);
    unsafe { advise_huge_pages(vec.as_mut_ptr().cast(), size_in_elements * 64, huge_pages) };
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
    order.shuffle(&mut rand::rng());

    RandomMemoryTest { vec, order, i: 0 }
}

fn legacy_black_box<T>(dummy: T) -> T {
    unsafe {
        let ret = ptr::read_volatile(&dummy);
        forget(dummy);
        ret
    }
}

// One 64-byte cache line per node. Only the first word is used as the "next"
// pointer; the rest pads the node out so every hop touches a new line.
type Node = [usize; 8];

// Unlike the `memory_random` group, every load here depends on the previous one, so the
// CPU cannot overlap misses. This is load-to-use latency, the number people
// usually mean by "a random memory access costs ~100 ns".
pub(super) struct PointerChase {
    ptr: *mut Node,
    pub(super) len: usize,
    bytes: usize,
}

impl PointerChase {
    pub(super) fn new(bytes: usize, huge_pages: bool) -> Self {
        let len = bytes / std::mem::size_of::<Node>();
        let ptr = unsafe {
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                bytes,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );
            assert!(ptr != libc::MAP_FAILED, "mmap failed");
            advise_huge_pages(ptr, bytes, huge_pages);
            ptr as *mut Node
        };

        // Sattolo's algorithm: a random permutation that is a single cycle, so
        // the chase visits every line before repeating.
        let mut order: Vec<usize> = (0..len).collect();
        let mut rng = StdRng::seed_from_u64(0xc0ffee);
        for i in (1..len).rev() {
            let j = rng.random_range(0..i);
            order.swap(i, j);
        }
        for i in 0..len {
            unsafe {
                (*ptr.add(i))[0] = order[i];
            }
        }

        Self { ptr, len, bytes }
    }

    fn point_at_self(&self) {
        for i in 0..self.len {
            unsafe {
                (*self.ptr.add(i))[0] = i;
            }
        }
    }

    #[inline(never)]
    pub(super) fn chase_write(&self, start: usize, steps: u64) -> usize {
        use std::sync::atomic::{AtomicU64, Ordering};
        let mut idx = start;
        for _ in 0..steps {
            let word = unsafe { &*(self.ptr.add(idx) as *const AtomicU64) };
            let old = word.fetch_add(1 << 32, Ordering::Relaxed);
            idx = (old & 0xffff_ffff) as usize;
        }
        idx
    }

    // Every line points at the next one, so the prefetchers run ahead of the
    // chase: the latency of one sequential read, one at a time.
    pub(super) fn sequential(bytes: usize, huge_pages: bool) -> Self {
        let chase = Self::new(bytes, huge_pages);
        for i in 0..chase.len {
            unsafe {
                (*chase.ptr.add(i))[0] = (i + 1) % chase.len;
            }
        }
        chase
    }

    #[inline(never)]
    pub(super) fn chase(&self, start: usize, steps: u64) -> usize {
        let mut idx = start;
        for _ in 0..steps {
            idx = unsafe { (*self.ptr.add(idx))[0] };
        }
        idx
    }
}

impl Drop for PointerChase {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr as *mut libc::c_void, self.bytes);
        }
    }
}

#[cfg(target_os = "linux")]
pub(super) unsafe fn advise_huge_pages(ptr: *mut libc::c_void, bytes: usize, huge_pages: bool) {
    let advice = if huge_pages {
        libc::MADV_HUGEPAGE
    } else {
        libc::MADV_NOHUGEPAGE
    };
    libc::madvise(ptr, bytes, advice);
}

#[cfg(not(target_os = "linux"))]
pub(super) unsafe fn advise_huge_pages(_ptr: *mut libc::c_void, _bytes: usize, _huge_pages: bool) {}

// Load-to-use latency: a dependent pointer chase, one hop per sample step.
pub(super) fn chase_group(c: &mut Criterion, group_name: &str, shapes: &[(&str, usize, bool)]) {
    let mut group = c.benchmark_group(group_name);
    for (name, bytes, huge_pages) in shapes.iter() {
        let chase = PointerChase::new(*bytes, *huge_pages);
        // Touch the whole cycle once so page faults are not measured.
        let mut idx = chase.chase(0, chase.len as u64);
        group.bench_function(*name, |b| {
            b.iter_custom(|iters| {
                let start = Instant::now();
                idx = chase.chase(idx, iters);
                start.elapsed()
            })
        });
        black_box(idx);
    }
    group.finish();
}

fn memory_random_read_latency_benchmark(c: &mut Criterion) {
    chase_group(
        c,
        "memory_random_read_latency",
        &[
            ("1 GiB, huge pages", GIB, true),
            ("1 GiB, 4 KiB pages", GIB, false),
        ],
    );
}

fn memory_sequential_read_latency_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_sequential_read_latency");
    for (name, huge_pages) in [("1 GiB, huge pages", true), ("1 GiB, 4 KiB pages", false)] {
        let chase = PointerChase::sequential(GIB, huge_pages);
        let mut idx = chase.chase(0, chase.len as u64);
        group.bench_function(name, |b| {
            b.iter_custom(|iters| {
                let start = Instant::now();
                idx = chase.chase(idx, iters);
                start.elapsed()
            })
        });
        black_box(idx);
    }
    group.finish();
}

// One chase per core at the same time, 512 MiB each on huge pages, so the latency is what each
// core sees with every other core missing too. Each thread builds its own, on its own core.
fn memory_latency_all_cores_benchmark(c: &mut Criterion) {
    let cores = core_affinity::get_core_ids().unwrap();
    let mut group = c.benchmark_group("memory_latency_all_cores");
    group.sample_size(10);
    for (name, sequential, write) in [
        ("random", false, false),
        ("sequential", true, false),
        ("random write", false, true),
        ("sequential write", true, true),
    ] {
        let chases: Vec<PointerChase> = std::thread::scope(|s| {
            let handles: Vec<_> = cores
                .iter()
                .map(|core| {
                    let core = *core;
                    s.spawn(move || {
                        core_affinity::set_for_current(core);
                        let chase = if sequential {
                            PointerChase::sequential(512 << 20, true)
                        } else {
                            PointerChase::new(512 << 20, true)
                        };
                        black_box(chase.chase(0, chase.len as u64));
                        chase
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        // Each thread carries on where it stopped, so repeated samples keep walking cold lines.
        let positions: Vec<std::sync::atomic::AtomicUsize> = (0..cores.len())
            .map(|_| std::sync::atomic::AtomicUsize::new(0))
            .collect();
        let positions = &positions;
        group.bench_function(name, |b| {
            b.iter_custom(|iters| {
                let start = Instant::now();
                std::thread::scope(|s| {
                    for (t, (chase, core)) in chases.iter().zip(cores.iter().copied()).enumerate() {
                        s.spawn(move || {
                            core_affinity::set_for_current(core);
                            let at = positions[t].load(std::sync::atomic::Ordering::Relaxed);
                            let at = if write {
                                chase.chase_write(at, iters)
                            } else {
                                chase.chase(at, iters)
                            };
                            positions[t].store(at, std::sync::atomic::Ordering::Relaxed);
                        });
                    }
                });
                start.elapsed()
            })
        });
    }
    group.finish();
}

// How long a write takes when the thread has to wait for it: an atomic add on the line, which only
// finishes once this core owns the line. A plain store retires into the store buffer, and a store
// plus a fence doesn't wait either: 8 random bytes then a fence took 10-15 ns on all three CPUs,
// a tenth of a miss. The chase's next index is in the low half of the word, so each write depends
// on the one before it. "same line" is the atomic alone, on a line this core already owns.
fn memory_write_latency_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_write_latency");
    let chases = [
        ("same line", PointerChase::new(4096, true), 1),
        ("sequential", PointerChase::sequential(GIB, true), 0),
        ("random", PointerChase::new(GIB, true), 0),
    ];
    for (name, chase, self_loop) in chases.iter() {
        if *self_loop == 1 {
            chase.point_at_self();
        }
        let mut idx = chase.chase_write(0, chase.len as u64);
        group.bench_function(*name, |b| {
            b.iter_custom(|iters| {
                let start = Instant::now();
                idx = chase.chase_write(idx, iters);
                start.elapsed()
            })
        });
        black_box(idx);
    }
    group.finish();
}

// One random block at a time, each read depending on every word of the block before: the latency
// of a random read against its size. Only the first word of a block is non-zero, the next block's
// index, so the sum of the block is where to go next.
fn memory_random_block_latency_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_random_block_latency");
    for block in [64usize, 128, 256, 512, 1 << 10, 4 << 10, 16 << 10] {
        let chase = PointerChase::new(GIB, true);
        let words = block / 8;
        let blocks = GIB / block;
        let base = chase.ptr as *mut u64;
        let mut order: Vec<usize> = (0..blocks).collect();
        let mut rng = StdRng::seed_from_u64(0xb10c);
        for i in (1..blocks).rev() {
            let j = rng.random_range(0..i);
            order.swap(i, j);
        }
        unsafe { ptr::write_bytes(base, 0, GIB / 8) };
        for (i, next) in order.iter().enumerate() {
            unsafe { base.add(i * words).write(*next as u64) };
        }
        let walk = |mut at: usize, steps: u64| {
            for _ in 0..steps {
                let words = unsafe { std::slice::from_raw_parts(base.add(at * words), words) };
                at = black_box(words.iter().fold(0u64, |a, b| a.wrapping_add(*b))) as usize;
            }
            at
        };
        let mut at = walk(0, blocks as u64);
        group.throughput(Throughput::Bytes(block as u64));
        group.bench_function(BenchmarkId::from_parameter(block), |b| {
            b.iter_custom(|iters| {
                let start = Instant::now();
                at = walk(at, iters);
                start.elapsed()
            })
        });
    }
    group.finish();
}

// All cores reading or writing random 64-byte lines of one shared 16 GiB buffer, far larger
// than any L3. Each pinned thread draws its own indices from a xorshift generator: an index
// array for 16 GiB would be 2 GiB of extra traffic, and the generator doesn't depend on the
// loaded data, so the misses still overlap.
struct SharedLines {
    ptr: *mut CacheLine,
    len: usize,
}

unsafe impl Send for PointerChase {}
unsafe impl Sync for PointerChase {}

unsafe impl Sync for SharedLines {}
unsafe impl Send for SharedLines {}

const SHARED_BYTES: usize = 16 * GIB;
const ACCESSES_PER_THREAD: usize = 1 << 22;

impl SharedLines {
    fn new(huge_pages: bool, cores: &[core_affinity::CoreId]) -> Self {
        let layout = std::alloc::Layout::from_size_align(SHARED_BYTES, 2 << 20).unwrap();
        let ptr = unsafe { std::alloc::alloc(layout) as *mut CacheLine };
        unsafe { advise_huge_pages(ptr.cast(), SHARED_BYTES, huge_pages) };
        let lines = SharedLines { ptr, len: SHARED_BYTES / 64 };
        assert!(lines.len.is_power_of_two());
        // Fill in parallel, so first touch spreads the pages like a real workload would.
        let per = lines.len / cores.len();
        std::thread::scope(|s| {
            for (t, core) in cores.iter().enumerate() {
                let lines = &lines;
                s.spawn(move || {
                    core_affinity::set_for_current(*core);
                    for i in t * per..(t + 1) * per {
                        unsafe { lines.ptr.add(i).write([i as u64; 8]) };
                    }
                });
            }
        });
        lines
    }

    // Random blocks instead of lines: 256 MiB per thread, whatever the size.
    fn run_blocks(&self, cores: &[core_affinity::CoreId], write: bool, block: usize) -> std::time::Duration {
        let start = Instant::now();
        let bytes = self.len * 64;
        std::thread::scope(|s| {
            for (t, core) in cores.iter().enumerate() {
                s.spawn(move || {
                    core_affinity::set_for_current(*core);
                    let mut x = 0x9E3779B97F4A7C15u64 ^ (t as u64).wrapping_mul(0xD1B54A32D192ED03);
                    let mut sum = 0u64;
                    for _ in 0..ACCESSES_PER_THREAD * 64 / block {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        let at = (x as usize & (bytes - 1)) & !(block - 1);
                        let p = unsafe { (self.ptr as *mut u8).add(at) };
                        if write {
                            unsafe { ptr::write_bytes(p, x as u8, block) };
                        } else {
                            let words = unsafe { std::slice::from_raw_parts(p as *const u64, block / 8) };
                            sum = words.iter().fold(sum, |a, b| a.wrapping_add(*b));
                        }
                    }
                    black_box(sum);
                });
            }
        });
        start.elapsed()
    }

    fn run(&self, cores: &[core_affinity::CoreId], write: bool) -> std::time::Duration {
        let start = Instant::now();
        std::thread::scope(|s| {
            for (t, core) in cores.iter().enumerate() {
                s.spawn(move || {
                    core_affinity::set_for_current(*core);
                    let mut x = 0x9E3779B97F4A7C15u64 ^ (t as u64).wrapping_mul(0xD1B54A32D192ED03);
                    let mut sum = 0u64;
                    for _ in 0..ACCESSES_PER_THREAD {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        let line = unsafe { self.ptr.add(x as usize & (self.len - 1)) };
                        if write {
                            unsafe { ptr::write_volatile(line, [x; 8]) };
                        } else {
                            sum = sum.wrapping_add(unsafe { ptr::read_volatile(line) }[0]);
                        }
                    }
                    black_box(sum);
                });
            }
        });
        start.elapsed()
    }
}

impl Drop for SharedLines {
    fn drop(&mut self) {
        let layout = std::alloc::Layout::from_size_align(SHARED_BYTES, 2 << 20).unwrap();
        unsafe { std::alloc::dealloc(self.ptr.cast(), layout) };
    }
}

fn memory_random_threads_benchmark(c: &mut Criterion) {
    let cores = core_affinity::get_core_ids().unwrap();
    let mut group = c.benchmark_group("memory_random");
    group.sample_size(10);
    for huge_pages in [false, true] {
        let lines = SharedLines::new(huge_pages, &cores);
        let pages = if huge_pages { ", huge pages" } else { "" };
        for threads in [&cores[..1], &cores[..]] {
            group.throughput(Throughput::Bytes((threads.len() * ACCESSES_PER_THREAD * 64) as u64));
            for (op, write) in [("read", false), ("write", true)] {
                let n = threads.len();
                let name = format!("{op}, 16 GiB, {n} thread{}{pages}", if n == 1 { "" } else { "s" });
                group.bench_function(name, |b| {
                    b.iter_custom(|iters| (0..iters).map(|_| lines.run(threads, write)).sum())
                });
            }
        }
    }
    group.finish();
}

// Throughput of random reads and writes against their size, on huge pages.
fn memory_random_block_benchmark(c: &mut Criterion) {
    let cores = core_affinity::get_core_ids().unwrap();
    let lines = SharedLines::new(true, &cores);
    let mut group = c.benchmark_group("memory_random_block");
    group.sample_size(10);
    for block in [64usize, 256, 1 << 10, 4 << 10, 64 << 10, 1 << 20] {
        for threads in [&cores[..1], &cores[..]] {
            group.throughput(Throughput::Bytes((threads.len() * ACCESSES_PER_THREAD * 64) as u64));
            for (op, write) in [("read", false), ("write", true)] {
                group.bench_function(format!("{op}, {block} B, {} threads", threads.len()), |b| {
                    b.iter_custom(|iters| (0..iters).map(|_| lines.run_blocks(threads, write, block)).sum())
                });
            }
        }
    }
    group.finish();
}

fn memory_random_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_random");
    group.sample_size(10);
    group.throughput(Throughput::Bytes(GIB as u64));

    for (name, huge_pages) in [("read", false), ("read, huge pages", true)] {
        group.bench_function(name, |b| {
            b.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;
                for _ in 0..iters {
                    let mut test = random_memory_read_setup(huge_pages);
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

criterion_group!(
    benches,
    memory_random_benchmark,
    memory_random_threads_benchmark,
    memory_random_read_latency_benchmark,
    memory_sequential_read_latency_benchmark,
    memory_latency_all_cores_benchmark,
    memory_write_latency_benchmark,
    memory_random_block_latency_benchmark,
    memory_random_block_benchmark
);
