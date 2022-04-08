use criterion::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

#[macro_use]
extern crate byte_unit;

extern crate core_affinity;

#[inline(never)]
#[no_mangle]
fn memory_read_sequential_single_thread(vec: &Vec<u64>) -> u64 {
    // SHE-COM memory_read_sequential_single_thread
    let mut sum = 0;
    for el in vec {
        sum += el
    }

    sum
}

fn criterion_benchmark(c: &mut Criterion) {
    let bytes_per_iteration = 8;
    let size_bytes = n_gib_bytes!(1);
    let size_in_elements = (size_bytes as u64 / bytes_per_iteration) as u64;
    let mut vec: Vec<u64> = Vec::new();
    for i in 0..size_in_elements {
        vec.push(i)
    }

    let all_core_ids = Arc::new(core_affinity::get_core_ids().unwrap());
    // to make it easy to grab a subset if needed, e.g. on M1 you have low performance cores.
    let core_ids = all_core_ids.clone();
    let last_id = core_ids.last().unwrap().id;
    let num_cores = core_ids.len();

    let start = Arc::new(Barrier::new(core_ids.len() + 1));
    let end = Arc::new(Barrier::new(core_ids.len() + 1));
    let done_allocating = Arc::new(Barrier::new(core_ids.len() + 1));
    let threaded_total = Arc::new(AtomicU64::new(0));

    for core in core_ids.iter() {
        let t_start = start.clone();
        let t_end = end.clone();
        let t_total = threaded_total.clone();
        let t_done_allocating = done_allocating.clone();
        let core = core.clone();

        thread::spawn(move || {
            // Ensure each thread is scheduled on a different core to maximimize memory bandwidth.
            // Each core can typically only have ~10 outstanding requests to the L1 cache.
            // https://news.ycombinator.com/item?id=16174813
            //
            // An M1 CPU seems to be able to maximize bandwidth though, due to higher number of
            // outstanding requests to the L1 cache.
            core_affinity::set_for_current(core);

            let mut vec: Vec<u64> = Vec::new();
            let slice_size = size_in_elements / num_cores as u64;
            let mut range = (slice_size as u64 * core.id as u64)
                ..(slice_size as u64 * (core.id as u64 + 1));
            if core.id == last_id {
                // slice sizes might not entirely line up. doesn't really matter,
                // but nice to see that it's the same as in the single-threaded case.
                range.end = size_in_elements as u64;
            }
            // println!("\n{}: {:?} of {}\n", thread_id, range, size_in_elements);
            for i in range {
                vec.push(i)
            }

            t_done_allocating.wait();
            loop {
                // println!("{} Done allocating", core.id);
                t_start.wait();
                t_total.fetch_add(
                    memory_read_sequential_single_thread(&vec),
                    Ordering::Relaxed,
                );
                t_end.wait();
            }
        });
    }

    done_allocating.wait();

    let mut group = c.benchmark_group("memory_read");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(10));
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Bytes(size_bytes as u64));
    group.bench_function("1 thread", |b| {
        // b.iter(|| println!("{}", memory_read_sequential_single_thread(&vec)))
        b.iter(|| black_box(memory_read_sequential_single_thread(&vec)))
    });
    group.bench_function(format!("{} threads", core_ids.len()), |b| {
        b.iter(|| {
            threaded_total.store(0, Ordering::Relaxed);
            start.wait(); // every thread will start totalling...
            end.wait(); // wait for every thread to finish...
                        // println!("{:?}", threaded_total.load(Ordering::Relaxed));
        })
    });
    group.finish()
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
