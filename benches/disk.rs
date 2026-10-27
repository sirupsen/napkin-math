// SSD, Network Disk and HDD rows: sequential and random reads and writes with
// O_DIRECT, and buffered 8 KiB sequential reads and writes. No dependencies.
// Queue depth comes from threads: N threads each doing one blocking pread or
// pwrite at a time keep N requests in flight, a queue depth of N.
//
//   cargo bench --bench disk -- /mnt/disk/disk.dat                 # a file on a mounted file system
//   cargo bench --bench disk -- /dev/nvme1n1 --destroy /dev/nvme1n1   # an unused raw device
//   cargo bench --bench disk -- /dev/nvme1n1 --destroy /dev/nvme1n1 --size 1800g --no-precondition   # HDD
//
// The whole range is written first, because reads of never-written blocks can
// skip the flash; --no-precondition skips that for raw devices (an HDD has no
// such shortcut and takes hours to fill). Buffered reads drop the page cache
// first, which needs root.
use std::alloc::{alloc_zeroed, Layout};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const O_DIRECT: i32 = 0o40000;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const O_DIRECT: i32 = 0o200000;
#[cfg(not(target_os = "linux"))]
const O_DIRECT: i32 = 0;

const KIB: u64 = 1 << 10;
const MIB: u64 = 1 << 20;

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

    fn get_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.0, self.1) }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Read,
    Write,
}

#[derive(Clone, Copy, PartialEq)]
enum Pattern {
    Sequential,
    Random,
}

struct Test {
    name: &'static str,
    op: Op,
    pattern: Pattern,
    block: u64,
    threads: usize,
    direct: bool,
}

fn parse_size(s: &str) -> u64 {
    let s = s.to_ascii_lowercase();
    let (num, mult) = match s.chars().last().unwrap() {
        'k' => (&s[..s.len() - 1], KIB),
        'm' => (&s[..s.len() - 1], MIB),
        'g' => (&s[..s.len() - 1], 1 << 30),
        't' => (&s[..s.len() - 1], 1 << 40),
        _ => (&s[..], 1),
    };
    num.parse::<u64>().unwrap() * mult
}

fn open(path: &str, direct: bool) -> File {
    let mut opts = OpenOptions::new();
    opts.read(true).write(true);
    if direct {
        opts.custom_flags(O_DIRECT);
    }
    opts.open(path).unwrap_or_else(|e| panic!("open {}: {}", path, e))
}

fn drop_page_cache() {
    if std::fs::write("/proc/sys/vm/drop_caches", "1").is_err() {
        eprintln!("could not drop the page cache (not root?); buffered reads may hit memory");
    }
}

/// One test: `threads` workers, each with one request in flight, for `secs`
/// (or, with `once`, until the range has been covered a single time).
fn run(path: &str, size: u64, t: &Test, secs: u64, once: bool) {
    let file = Arc::new(open(path, t.direct));
    let next = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let blocks = size / t.block;
    let start = Instant::now();
    let workers: Vec<_> = (0..t.threads)
        .map(|i| {
            let (file, next, stop) = (file.clone(), next.clone(), stop.clone());
            let (op, pattern, block) = (t.op, t.pattern, t.block);
            std::thread::spawn(move || {
                let mut buf = Buf::new(block as usize);
                let mut rng = 0x9e3779b97f4a7c15u64 ^ (i as u64 + 1).wrapping_mul(0xbf58476d1ce4e5b9);
                let mut lat: Vec<u32> = Vec::with_capacity(1 << 16);
                while !stop.load(Ordering::Relaxed) {
                    let index = match pattern {
                        // All threads share one cursor, so together they read
                        // or write the range front to back.
                        Pattern::Sequential => {
                            let n = next.fetch_add(1, Ordering::Relaxed);
                            if n >= blocks && once {
                                break;
                            }
                            n % blocks
                        }
                        Pattern::Random => {
                            rng ^= rng << 13;
                            rng ^= rng >> 7;
                            rng ^= rng << 17;
                            rng % blocks
                        }
                    };
                    let begin = Instant::now();
                    match op {
                        Op::Read => file.read_exact_at(buf.get_mut(), index * block).unwrap(),
                        Op::Write => file.write_all_at(buf.get(), index * block).unwrap(),
                    }
                    lat.push(begin.elapsed().as_nanos().min(u32::MAX as u128) as u32);
                }
                lat
            })
        })
        .collect();
    if !once {
        std::thread::sleep(Duration::from_secs(secs));
        stop.store(true, Ordering::Relaxed);
    }
    let mut lat: Vec<u32> = workers.into_iter().flat_map(|w| w.join().unwrap()).collect();
    let elapsed = start.elapsed().as_secs_f64();
    if once {
        return;
    }
    lat.sort_unstable();
    let pct = |p: f64| lat[((lat.len() - 1) as f64 * p) as usize] as f64 / 1e3;
    let ops = lat.len() as f64 / elapsed;
    println!(
        "{:<22} {:>5} {:>4}  ops/s={:>9.0}  MiB/s={:>7.0}  p50={:>9.1}us  p99={:>9.1}us",
        t.name,
        if t.block >= MIB { format!("{}M", t.block / MIB) } else { format!("{}K", t.block / KIB) },
        format!("x{}", t.threads),
        ops,
        ops * t.block as f64 / MIB as f64,
        pct(0.5),
        pct(0.99)
    );
}

fn main() {
    let args: Vec<String> = std::env::args().filter(|a| a != "--bench").collect();
    let usage = "usage: disk <file | /dev/device --destroy /dev/device> [--size 32g] [--secs 15] [--no-precondition]";
    // `cargo bench --bench <name>` adds --bench to the arguments; without any
    // other arguments there is nothing to measure, so skip quietly.
    if args.len() < 2 {
        eprintln!("{} (no arguments, skipping)", usage);
        return;
    }
    let path = args.get(1).expect(usage).clone();
    let flag = |name: &str| args.iter().position(|a| a == name).map(|i| args[i + 1].clone());
    let secs: u64 = flag("--secs").map(|s| s.parse().unwrap()).unwrap_or(15);
    let mut size = parse_size(&flag("--size").unwrap_or("32g".into()));
    let precondition = !args.iter().any(|a| a == "--no-precondition");
    let device = path.starts_with("/dev/");

    if device {
        if flag("--destroy").as_deref() != Some(path.as_str()) {
            eprintln!("writing to {} destroys its contents; rerun with --destroy {}", path, path);
            std::process::exit(1);
        }
        let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
        if mounts.lines().any(|l| l.starts_with(&path)) {
            eprintln!("{} (or a partition on it) is mounted; refusing to write to it", path);
            std::process::exit(1);
        }
        let len = File::open(&path).unwrap().seek(SeekFrom::End(0)).unwrap();
        size = size.min(len) / MIB * MIB;
    } else {
        // Check the file system the file lands on, with 2x headroom: an
        // earlier run filled a boot disk because the "SSD array" path turned
        // out to be a symlink onto it.
        let dir = std::path::Path::new(&path).parent().unwrap().to_str().unwrap().to_string();
        let df = Command::new("df").args(["-Pk", if dir.is_empty() { "." } else { &dir }]).output().unwrap();
        let df = String::from_utf8_lossy(&df.stdout);
        let fields: Vec<&str> = df.lines().last().unwrap().split_whitespace().collect();
        let avail = fields[3].parse::<u64>().unwrap() * KIB;
        if avail < 2 * size {
            eprintln!("only {} GiB free on {}; need 2x {} GiB", avail >> 30, fields[5], size >> 30);
            std::process::exit(1);
        }
        println!("file system: {} on {}", fields[0], fields[5]);
        File::create(&path).unwrap().set_len(size).unwrap();
    }
    println!("{}: {} GiB, {} s per test", path, size >> 30, secs);

    let seq = |name, op, block, threads, direct| Test { name, op, pattern: Pattern::Sequential, block, threads, direct };
    let rand = |name, op, threads| Test { name, op, pattern: Pattern::Random, block: 8 * KIB, threads, direct: true };
    // A file always gets written first: reading a hole never touches the disk.
    if precondition || !device {
        run(&path, size, &seq("precondition", Op::Write, MIB, 32, true), 0, true);
    }
    run(&path, size, &seq("seq_read_1m_qd32", Op::Read, MIB, 32, true), secs, false);
    drop_page_cache();
    run(&path, size, &seq("seq_read_8k_buffered", Op::Read, 8 * KIB, 1, false), secs, false);
    run(&path, size, &rand("rand_read_8k_qd1", Op::Read, 1), secs, false);
    run(&path, size, &rand("rand_read_8k_qd128", Op::Read, 128), secs, false);
    run(&path, size, &rand("rand_write_8k_qd1", Op::Write, 1), secs, false);
    run(&path, size, &rand("rand_write_8k_qd128", Op::Write, 128), secs, false);
    run(&path, size, &seq("seq_write_1m_qd32", Op::Write, MIB, 32, true), secs, false);
    run(&path, size, &seq("seq_write_8k_buffered", Op::Write, 8 * KIB, 1, false), 5.min(secs), false);
    if !device {
        std::fs::remove_file(&path).unwrap();
    }
}
