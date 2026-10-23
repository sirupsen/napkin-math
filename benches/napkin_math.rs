mod benchmarks;
#[macro_use]
extern crate byte_unit;
extern crate jemallocator;
use criterion::criterion_main;

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

criterion_main! {
    benchmarks::compression::benches,
    benchmarks::context_switch::benches,
    benchmarks::memory_read::benches,
    benchmarks::memory_random::benches,
    benchmarks::memory_latency::benches,
    benchmarks::hash::benches,
    benchmarks::serialization::benches,
    benchmarks::sort::benches,
    benchmarks::syscall::benches,
    benchmarks::compressed_memory_read::benches,
}
