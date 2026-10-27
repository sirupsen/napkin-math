use criterion::*;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Instant;

// Cost of one atomic increment or one mutex lock + unlock, alone and while a
// second thread pinned to another core hammers the same cache line.
fn contended<T: Send + Sync + 'static>(
    shared: Arc<T>,
    cores: &[core_affinity::CoreId],
    op: fn(&T),
    iters: u64,
) -> std::time::Duration {
    let stop = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(Barrier::new(2));
    let (t_shared, t_stop, t_ready, t_core) =
        (shared.clone(), stop.clone(), ready.clone(), cores[1]);
    let other = thread::spawn(move || {
        core_affinity::set_for_current(t_core);
        t_ready.wait();
        while !t_stop.load(Ordering::Relaxed) {
            op(&t_shared);
        }
    });

    // Time on our own pinned thread too, so the two never share a core.
    let main_core = cores[0];
    let elapsed = thread::spawn(move || {
        core_affinity::set_for_current(main_core);
        ready.wait();
        let start = Instant::now();
        for _ in 0..iters {
            op(&shared);
        }
        let elapsed = start.elapsed();
        stop.store(true, Ordering::Relaxed);
        elapsed
    })
    .join()
    .unwrap();
    other.join().unwrap();
    elapsed
}

fn increment(counter: &AtomicU64) {
    black_box(counter.fetch_add(1, Ordering::SeqCst));
}

fn lock(mutex: &Mutex<u64>) {
    *mutex.lock().unwrap() += 1;
}

fn cpu_atomic_mutex_benchmark(c: &mut Criterion) {
    let cores = core_affinity::get_core_ids().unwrap();
    let mut group = c.benchmark_group("cpu_atomic_mutex");

    let counter = Arc::new(AtomicU64::new(0));
    group.bench_function("atomic increment", |b| b.iter(|| increment(&counter)));
    let mutex = Arc::new(Mutex::new(0u64));
    group.bench_function("mutex lock + unlock", |b| b.iter(|| lock(&mutex)));

    if cores.len() >= 2 {
        // CPUs 0 and 1 are different physical cores on Linux cloud VMs, where
        // SMT siblings are numbered n/2 apart.
        let pair = [cores[0], cores[1]];
        group.bench_function("atomic increment, contended", |b| {
            b.iter_custom(|iters| contended(counter.clone(), &pair, increment, iters))
        });
        group.bench_function("mutex lock + unlock, contended", |b| {
            b.iter_custom(|iters| contended(mutex.clone(), &pair, lock, iters))
        });
    }

    group.finish();
}

criterion_group!(benches, cpu_atomic_mutex_benchmark);
