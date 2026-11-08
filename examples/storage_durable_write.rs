// Durable writes: each writer does pwrite + fdatasync in a loop and times the
// pair, the way a database commits to its log. On Linux, --depths also runs one
// thread with that many write + fdatasync pairs in flight on io_uring, which is
// group commit from a single thread.
//
//   cargo run --release --example storage_durable_write -- /mnt/disk/commit.dat
//   cargo run --release --example storage_durable_write -- /mnt/disk/commit.dat --sizes 8k,64k,1m --writers 1,32 --secs 15
//
// Every writer overwrites its own 64 MiB slice of a file that is fully written
// and synced first, sequentially and wrapping around, with O_DIRECT on Linux.
// Overwriting written blocks keeps file system metadata out of fdatasync;
// --append measures the other case, where every fdatasync also has to persist
// the new file size. On Linux it also reports how many flushes and writes the
// device saw per commit (from /proc/diskstats), which shows whether fdatasync
// reached the device at all.
#[path = "common/disk.rs"]
mod disk;
use disk::{parse_size, Buf, O_DIRECT};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const SLICE: u64 = 64 << 20;

fn human(bytes: usize) -> String {
    if bytes >= 1 << 20 {
        format!("{} MiB", bytes >> 20)
    } else {
        format!("{} KiB", bytes >> 10)
    }
}

/// (flushes, writes) completed by the device holding `file`, or None off Linux
/// or when the device isn't in /proc/diskstats (e.g. a device mapper volume).
fn device_counters(file: &File) -> Option<(u64, u64)> {
    let dev = file.metadata().ok()?.dev();
    let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff);
    let minor = (dev & 0xff) | ((dev >> 12) & !0xff);
    // After major, minor and name: field 5 is writes completed, 16 flushes completed.
    let stats = std::fs::read_to_string("/proc/diskstats").ok()?;
    for line in stats.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() >= 20 && f[0].parse::<u64>().ok()? == major && f[1].parse::<u64>().ok()? == minor
        {
            return Some((f[18].parse().ok()?, f[7].parse().ok()?));
        }
    }
    None
}

fn open(path: &str, append: bool) -> File {
    let mut opts = OpenOptions::new();
    opts.read(true).write(true).custom_flags(O_DIRECT);
    if append {
        opts.create(true).truncate(true);
    }
    opts.open(path)
        .unwrap_or_else(|e| panic!("open {}: {}", path, e))
}

fn prepare(path: &str, len: u64) {
    let mut file = File::create(path).unwrap();
    let chunk = vec![0x5a; 1 << 20];
    for _ in 0..len >> 20 {
        file.write_all(&chunk).unwrap();
    }
    file.sync_all().unwrap();
}

struct Result {
    commits: u64,
    latencies: Vec<u32>,
    write_ns: u64,
    sync_ns: u64,
}

fn writer(file: Arc<File>, base: u64, size: usize, append: bool, stop: Arc<AtomicBool>) -> Result {
    let buf = Buf::new(size);
    let mut r = Result {
        commits: 0,
        latencies: Vec::with_capacity(1 << 20),
        write_ns: 0,
        sync_ns: 0,
    };
    let mut offset = 0;
    while !stop.load(Ordering::Relaxed) {
        if !append && offset + size as u64 > SLICE {
            offset = 0;
        }
        let start = Instant::now();
        file.write_all_at(buf.get(), base + offset).unwrap();
        let written = Instant::now();
        file.sync_data().unwrap();
        let done = Instant::now();
        offset += size as u64;
        r.commits += 1;
        r.write_ns += (written - start).as_nanos() as u64;
        r.sync_ns += (done - written).as_nanos() as u64;
        r.latencies
            .push((done - start).as_nanos().min(u32::MAX as u128) as u32);
    }
    r
}

/// One thread keeping `depth` commits in flight on io_uring, each a write
/// linked to an fdatasync, over its own 64 MiB slice: group commit from one
/// thread, the way a database log can do it.
#[cfg(target_os = "linux")]
fn uring_writer(file: &File, size: usize, depth: usize, stop: &AtomicBool) -> Result {
    use io_uring::{opcode, squeue, types, IoUring};
    use std::os::unix::io::AsRawFd;

    let mut ring = IoUring::new((2 * depth).next_power_of_two() as u32).expect("io_uring");
    ring.submitter().register_files(&[file.as_raw_fd()]).expect("register file");
    let fd = types::Fixed(0);
    let bufs: Vec<Buf> = (0..depth).map(|_| Buf::new(size)).collect();
    let mut offsets = vec![0u64; depth];
    let mut started = vec![Instant::now(); depth];
    let mut r = Result { commits: 0, latencies: Vec::with_capacity(1 << 20), write_ns: 0, sync_ns: 0 };
    let mut written = vec![Instant::now(); depth];
    let submit = |ring: &mut IoUring, slot: usize, offset: u64| {
        let write = opcode::Write::new(fd, bufs[slot].0, size as u32)
            .offset(slot as u64 * SLICE + offset)
            .build()
            .flags(squeue::Flags::IO_LINK)
            .user_data((slot * 2) as u64);
        let sync = opcode::Fsync::new(fd)
            .flags(types::FsyncFlags::DATASYNC)
            .build()
            .user_data((slot * 2 + 1) as u64);
        unsafe {
            ring.submission().push(&write).unwrap();
            ring.submission().push(&sync).unwrap();
        }
    };
    for (slot, start) in started.iter_mut().enumerate().take(depth) {
        *start = Instant::now();
        submit(&mut ring, slot, 0);
    }
    let mut in_flight = depth;
    let mut done = Vec::with_capacity(depth);
    while in_flight > 0 {
        ring.submit_and_wait(1).expect("io_uring submit");
        let now = Instant::now();
        for cqe in ring.completion() {
            let slot = cqe.user_data() as usize / 2;
            if cqe.user_data() % 2 == 0 {
                assert_eq!(cqe.result(), size as i32, "short or failed write");
                written[slot] = now;
                continue;
            }
            assert_eq!(cqe.result(), 0, "fdatasync failed");
            r.commits += 1;
            r.write_ns += (written[slot] - started[slot]).as_nanos() as u64;
            r.sync_ns += (now - written[slot]).as_nanos() as u64;
            r.latencies.push((now - started[slot]).as_nanos().min(u32::MAX as u128) as u32);
            done.push(slot);
        }
        let stopping = stop.load(Ordering::Relaxed);
        for slot in done.drain(..) {
            in_flight -= 1;
            if stopping {
                continue;
            }
            offsets[slot] += size as u64;
            if offsets[slot] + size as u64 > SLICE {
                offsets[slot] = 0;
            }
            started[slot] = Instant::now();
            submit(&mut ring, slot, offsets[slot]);
            in_flight += 1;
        }
    }
    r
}

/// `writers` threads with one commit in flight each, or with `depth` > 1 (Linux)
/// one thread with `depth` commits in flight on io_uring.
fn run(path: &str, size: usize, writers: usize, depth: usize, secs: u64, append: bool) {
    let file = Arc::new(open(path, append));
    let before = device_counters(&file);
    let stop = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    #[cfg(target_os = "linux")]
    let uring = if depth > 1 {
        let (file, stop) = (file.clone(), stop.clone());
        Some(std::thread::spawn(move || uring_writer(&file, size, depth, &stop)))
    } else {
        None
    };
    #[cfg(not(target_os = "linux"))]
    let uring: Option<std::thread::JoinHandle<Result>> = None;
    let writers = if uring.is_some() { 0 } else { writers };
    let handles: Vec<_> = (0..writers)
        .map(|i| {
            let (file, stop) = (file.clone(), stop.clone());
            // Appending writers would race for the end of the file; give each
            // one a region far apart instead (the file grows sparse between).
            let base = if append {
                i as u64 * (1 << 40)
            } else {
                i as u64 * SLICE
            };
            std::thread::spawn(move || writer(file, base, size, append, stop))
        })
        .collect();
    std::thread::sleep(Duration::from_secs(secs));
    stop.store(true, Ordering::Relaxed);
    let results: Vec<Result> = handles.into_iter().chain(uring).map(|h| h.join().unwrap()).collect();
    let elapsed = start.elapsed().as_secs_f64();
    let after = device_counters(&file);

    let commits: u64 = results.iter().map(|r| r.commits).sum();
    let mut lat: Vec<u32> = results
        .iter()
        .flat_map(|r| r.latencies.iter().copied())
        .collect();
    lat.sort_unstable();
    let pct = |p: f64| lat[((lat.len() - 1) as f64 * p) as usize] as f64 / 1e3;
    let write_us = results.iter().map(|r| r.write_ns).sum::<u64>() as f64 / commits as f64 / 1e3;
    let sync_us = results.iter().map(|r| r.sync_ns).sum::<u64>() as f64 / commits as f64 / 1e3;
    let rate = commits as f64 / elapsed;
    let device = match (before, after) {
        (Some(b), Some(a)) => format!(
            "  device flushes/commit={:.2} writes/commit={:.2}",
            (a.0 - b.0) as f64 / commits as f64,
            (a.1 - b.1) as f64 / commits as f64
        ),
        _ => String::new(),
    };
    println!(
        "{:<9} {:>7} {:<6} commits/s={:>8.0}  MiB/s={:>6.0}  p50={:>8.1}us  p99={:>8.1}us  write={:>7.1}us  fdatasync={:>7.1}us{}",
        if append { "append" } else { "overwrite" },
        human(size),
        if writers == 0 { format!("1x{}", depth) } else { format!("x{}", writers) },
        rate,
        rate * size as f64 / (1 << 20) as f64,
        pct(0.5),
        pct(0.99),
        write_us,
        sync_us,
        device
    );
    if append {
        std::fs::remove_file(path).unwrap();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let usage =
        "usage: storage_durable_write <file> [--sizes 8k,64k,1m] [--writers 1,32] [--depths 32] [--secs 15]";
    if args.len() < 2 {
        eprintln!("{usage}");
        std::process::exit(2);
    }
    let path = args[1].clone();
    let flag = |name: &str, default: &str| -> String {
        args.iter()
            .position(|a| a == name)
            .map(|i| args[i + 1].clone())
            .unwrap_or(default.to_string())
    };
    let sizes: Vec<usize> = flag("--sizes", "8k,64k,1m")
        .split(',')
        .map(|s| parse_size(s) as usize)
        .collect();
    let writers: Vec<usize> = flag("--writers", "1,32")
        .split(',')
        .map(|w| w.parse().unwrap())
        .collect();
    // One io_uring thread with this many commits in flight (Linux only).
    let depths: Vec<usize> = if cfg!(target_os = "linux") {
        flag("--depths", "32").split(',').map(|d| d.parse().unwrap()).collect()
    } else {
        vec![]
    };
    let secs: u64 = flag("--secs", "15").parse().unwrap();

    let slices = writers.iter().chain(&depths).max().copied().unwrap();
    prepare(&path, SLICE * slices as u64);
    for &size in &sizes {
        for &w in &writers {
            run(&path, size, w, 1, secs, false);
        }
        for &d in &depths {
            run(&path, size, 1, d, secs, false);
        }
    }
    let append_path = format!("{}.append", path);
    for &size in &sizes {
        run(&append_path, size, 1, 1, secs, true);
    }
    std::fs::remove_file(&path).unwrap();
}
