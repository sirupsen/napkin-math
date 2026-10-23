mod benchmarks;
use criterion::criterion_main;

#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

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
    benchmarks::tcp::benches,
    benchmarks::compressed_memory_read::benches,
}
