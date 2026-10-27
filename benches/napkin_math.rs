mod benchmarks;
use criterion::criterion_main;

#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

criterion_main! {
    benchmarks::cache_read::benches,
    benchmarks::cpu_atomic_mutex::benches,
    benchmarks::cpu_context_switch::benches,
    benchmarks::cpu_encrypt::benches,
    benchmarks::cpu_handshake::benches,
    benchmarks::cpu_hash::benches,
    benchmarks::cpu_serialize::benches,
    benchmarks::cpu_sort::benches,
    benchmarks::cpu_spawn::benches,
    benchmarks::cpu_system_call::benches,
    benchmarks::memory_random::benches,
    benchmarks::memory_sequential_read::benches,
    benchmarks::memory_sequential_write::benches,
}
