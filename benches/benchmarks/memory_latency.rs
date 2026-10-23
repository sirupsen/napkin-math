use criterion::*;
use std::hint::black_box;

use super::{GIB, KIB, MIB};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use std::time::Instant;

// One 64-byte cache line per node. Only the first word is used as the "next"
// pointer; the rest pads the node out so every hop touches a new line.
type Node = [usize; 8];

// Unlike `memory_random`, every load here depends on the previous one, so the
// CPU cannot overlap misses. This is load-to-use latency, the number people
// usually mean by "a random memory access costs ~100 ns".
struct PointerChase {
    ptr: *mut Node,
    len: usize,
    bytes: usize,
}

impl PointerChase {
    fn new(bytes: usize, huge_pages: bool) -> Self {
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

    #[inline(never)]
    fn chase(&self, start: usize, steps: u64) -> usize {
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
unsafe fn advise_huge_pages(ptr: *mut libc::c_void, bytes: usize, huge_pages: bool) {
    let advice = if huge_pages {
        libc::MADV_HUGEPAGE
    } else {
        libc::MADV_NOHUGEPAGE
    };
    libc::madvise(ptr, bytes, advice);
}

#[cfg(not(target_os = "linux"))]
unsafe fn advise_huge_pages(_ptr: *mut libc::c_void, _bytes: usize, _huge_pages: bool) {}

fn memory_latency_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_latency");

    let shapes = [
        ("16 KiB", 16 * KIB, true),
        ("256 KiB", 256 * KIB, true),
        ("8 MiB", 8 * MIB, true),
        ("1 GiB, huge pages", GIB, true),
        ("1 GiB, 4 KiB pages", GIB, false),
    ];

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

criterion_group!(benches, memory_latency_benchmark);
