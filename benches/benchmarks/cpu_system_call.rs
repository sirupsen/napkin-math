use criterion::*;
use std::fs;
use std::hint::black_box;
use std::process;
use std::time::Instant;
use std::time::SystemTime;

// A real kernel round trip that does almost no work: glibc's getppid is a bare
// syscall wrapper, never served from the vDSO or a libc cache like
// clock_gettime and (on some libc versions) getpid.
#[inline(never)]
fn getppid_loop(iters: u64) -> std::time::Duration {
    let start = Instant::now();
    for _ in 0..iters {
        unsafe {
            black_box(libc::getppid());
        }
    }
    start.elapsed()
}

#[inline(never)]
fn getrusage_loop(ptr: *mut libc::rusage, iters: u64) -> std::time::Duration {
    let start = Instant::now();
    for _ in 0..iters {
        unsafe {
            libc::getrusage(0, ptr);
        }
    }
    start.elapsed()
}

fn cpu_system_call_benchmark(c: &mut Criterion) {
    let tmp = fs::File::open("/tmp").unwrap();
    let usage = Box::new(unsafe { std::mem::zeroed::<libc::rusage>() });
    let usage_ptr = Box::into_raw(usage);

    if std::env::var_os("NAPKIN_DEBUG_THREADS").is_some() {
        if let Ok(entries) = fs::read_dir("/proc/self/task") {
            eprintln!("syscall benchmark threads: {}", entries.count());
        }
    }

    let mut group = c.benchmark_group("cpu_system_call");
    group.bench_function("getpid", |b| b.iter(|| black_box(process::id())));
    group.bench_function("getppid", |b| b.iter_custom(getppid_loop));
    group.bench_function("time", |b| b.iter(|| black_box(SystemTime::now())));
    group.bench_function("getrusage", |b| {
        b.iter_custom(|iters| getrusage_loop(usage_ptr, iters))
    });
    group.bench_function("stat", |b| b.iter(|| black_box(tmp.metadata().unwrap())));
    group.finish();
}

criterion_group!(benches, cpu_system_call_benchmark);
