use criterion::*;

// Two threads hand a token back and forth through a futex. Each iteration is
// one round trip, i.e. two wake-ups. Pinned to the same CPU, every wake-up
// forces a context switch (lmbench `lat_ctx` / tsunanet style). Pinned to two
// different cores, it instead measures cross-core wake-up latency.
#[cfg(target_os = "linux")]
mod futex {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{Duration, Instant};

    fn wait(state: &AtomicU32, expected: u32) {
        unsafe {
            libc::syscall(
                libc::SYS_futex,
                state as *const AtomicU32,
                libc::FUTEX_WAIT | libc::FUTEX_PRIVATE_FLAG,
                expected,
                std::ptr::null::<libc::timespec>(),
            );
        }
    }

    fn wake(state: &AtomicU32) {
        unsafe {
            libc::syscall(
                libc::SYS_futex,
                state as *const AtomicU32,
                libc::FUTEX_WAKE | libc::FUTEX_PRIVATE_FLAG,
                1,
            );
        }
    }

    pub fn ping_pong(
        iters: u64,
        ping_core: core_affinity::CoreId,
        pong_core: core_affinity::CoreId,
    ) -> Duration {
        // 1 = pong's turn, 0 = ping's turn.
        let state = Arc::new(AtomicU32::new(0));
        let ready = Arc::new(Barrier::new(2));

        let pong_state = state.clone();
        let pong_ready = ready.clone();
        let pong = thread::spawn(move || {
            core_affinity::set_for_current(pong_core);
            pong_ready.wait();
            for _ in 0..iters {
                while pong_state.load(Ordering::Acquire) != 1 {
                    wait(&pong_state, 0);
                }
                pong_state.store(0, Ordering::Release);
                wake(&pong_state);
            }
        });

        // Run the timed side on its own thread too, so the benchmark's main
        // thread never ends up pinned for the groups that run after this one.
        let ping = thread::spawn(move || {
            core_affinity::set_for_current(ping_core);
            ready.wait();
            let start = Instant::now();
            for _ in 0..iters {
                state.store(1, Ordering::Release);
                wake(&state);
                while state.load(Ordering::Acquire) != 0 {
                    wait(&state, 1);
                }
            }
            start.elapsed()
        });

        let elapsed = ping.join().unwrap();
        pong.join().unwrap();
        elapsed
    }
}

#[cfg(target_os = "linux")]
fn cpu_context_switch_benchmark(c: &mut Criterion) {
    let cores = core_affinity::get_core_ids().unwrap();
    let first = cores[0];
    let last = *cores.last().unwrap();

    let mut group = c.benchmark_group("cpu_context_switch");
    group.bench_function("futex round trip, same core", |b| {
        b.iter_custom(|iters| futex::ping_pong(iters, first, first))
    });
    group.bench_function("futex round trip, cross core", |b| {
        b.iter_custom(|iters| futex::ping_pong(iters, first, last))
    });
    group.finish();
}

#[cfg(not(target_os = "linux"))]
fn cpu_context_switch_benchmark(_c: &mut Criterion) {}

criterion_group!(benches, cpu_context_switch_benchmark);
