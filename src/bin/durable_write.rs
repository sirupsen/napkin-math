// Durable writes: each writer does pwrite + fdatasync in a loop and times the
// pair, the way a database commits to its log. No fio, no dependencies, so it
// is easy to read exactly what is measured.
//
//   cargo run --release --bin durable_write -- /mnt/disk/commit.dat
//   cargo run --release --bin durable_write -- /mnt/disk/commit.dat --sizes 8k,64k,1m --writers 1,32 --secs 15
//
// Every writer overwrites its own 64 MiB slice of a file that is fully written
// and synced first, sequentially and wrapping around, with O_DIRECT on Linux.
// Overwriting written blocks keeps file system metadata out of fdatasync;
// --append measures the other case, where every fdatasync also has to persist
// the new file size. On Linux it also reports how many flushes and writes the
// device saw per commit (from /proc/diskstats), which shows whether fdatasync
// reached the device at all.
use std::alloc::{alloc_zeroed, Layout};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const SLICE: u64 = 64 << 20;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const O_DIRECT: i32 = 0o40000;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const O_DIRECT: i32 = 0o200000;
#[cfg(not(target_os = "linux"))]
const O_DIRECT: i32 = 0;

struct Buf(*mut u8, usize);
unsafe impl Send for Buf {}

impl Buf {
    fn new(len: usize) -> Buf {
        let ptr = unsafe { alloc_zeroed(Layout::from_size_align(len, 4096).unwrap()) };
        // Not all zeroes, so no layer can skip or compress the write.
        let slice = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
        for (i, b) in slice.iter_mut().enumerate() {
            *b = (i * 31 + 7) as u8;
        }
        Buf(ptr, len)
    }

    fn get(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.0, self.1) }
    }
}

fn parse_size(s: &str) -> usize {
    let s = s.to_ascii_lowercase();
    let (num, mult) = match s.chars().last().unwrap() {
        'k' => (&s[..s.len() - 1], 1 << 10),
        'm' => (&s[..s.len() - 1], 1 << 20),
        _ => (&s[..], 1),
    };
    num.parse::<usize>().unwrap() * mult
}

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
        if f.len() >= 20 && f[0].parse::<u64>().ok()? == major && f[1].parse::<u64>().ok()? == minor {
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
    opts.open(path).unwrap_or_else(|e| panic!("open {}: {}", path, e))
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
    let mut r = Result { commits: 0, latencies: Vec::with_capacity(1 << 20), write_ns: 0, sync_ns: 0 };
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
        r.latencies.push((done - start).as_nanos().min(u32::MAX as u128) as u32);
    }
    r
}

fn run(path: &str, size: usize, writers: usize, secs: u64, append: bool) {
    let file = Arc::new(open(path, append));
    let before = device_counters(&file);
    let stop = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    let handles: Vec<_> = (0..writers)
        .map(|i| {
            let (file, stop) = (file.clone(), stop.clone());
            // Appending writers would race for the end of the file; give each
            // one a region far apart instead (the file grows sparse between).
            let base = if append { i as u64 * (1 << 40) } else { i as u64 * SLICE };
            std::thread::spawn(move || writer(file, base, size, append, stop))
        })
        .collect();
    std::thread::sleep(Duration::from_secs(secs));
    stop.store(true, Ordering::Relaxed);
    let results: Vec<Result> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let elapsed = start.elapsed().as_secs_f64();
    let after = device_counters(&file);

    let commits: u64 = results.iter().map(|r| r.commits).sum();
    let mut lat: Vec<u32> = results.iter().flat_map(|r| r.latencies.iter().copied()).collect();
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
        "{:<9} {:>7} x{:<3} commits/s={:>8.0}  MiB/s={:>6.0}  p50={:>8.1}us  p99={:>8.1}us  write={:>7.1}us  fdatasync={:>7.1}us{}",
        if append { "append" } else { "overwrite" },
        human(size),
        writers,
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
    let path = args.get(1).expect("usage: durable_write <file> [--sizes 8k,64k,1m] [--writers 1,32] [--secs 15]").clone();
    let flag = |name: &str, default: &str| -> String {
        args.iter().position(|a| a == name).map(|i| args[i + 1].clone()).unwrap_or(default.to_string())
    };
    let sizes: Vec<usize> = flag("--sizes", "8k,64k,1m").split(',').map(parse_size).collect();
    let writers: Vec<usize> = flag("--writers", "1,32").split(',').map(|w| w.parse().unwrap()).collect();
    let secs: u64 = flag("--secs", "15").parse().unwrap();

    prepare(&path, SLICE * *writers.iter().max().unwrap() as u64);
    for &size in &sizes {
        for &w in &writers {
            run(&path, size, w, secs, false);
        }
    }
    let append_path = format!("{}.append", path);
    for &size in &sizes {
        run(&append_path, size, 1, secs, true);
    }
    std::fs::remove_file(&path).unwrap();
}
