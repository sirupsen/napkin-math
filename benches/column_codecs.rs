// Lightweight columnar encodings, per column of real data: ALP / ALP-RD for
// floats, frame of reference or delta + FastLanes bitpacking for integers, and
// FSST for strings. These are the encodings Vortex and DuckDB use, from the
// crates Vortex builds on. Every column is round-tripped and checked bit for
// bit, and decode is timed single-threaded (best of 5) against the decoded
// size. Ratios are against the Arrow in-memory size, with nulls dropped:
// 8 bytes per f64, the type's width per integer, and the bytes plus a 4-byte
// offset per string. zstd -3 over the same bytes is printed for reference.
//
//   cargo bench --features encodings --bench column_codecs -- floats taxi.parquet
//   cargo bench --features encodings --bench column_codecs -- ints hits_0.parquet
//   cargo bench --features encodings --bench column_codecs -- strings hits_0.parquet logs.txt
use std::fs::File;
use std::time::Instant;

use arrow_array::cast::AsArray;
use arrow_array::types::*;
use arrow_array::ArrayRef;
use fastlanes::{BitPacking, Delta, FoR, Transpose};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

const CHUNK: usize = 1024;

fn best_of<F: FnMut()>(runs: usize, mut f: F) -> f64 {
    (0..runs)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed().as_secs_f64()
        })
        .fold(f64::MAX, f64::min)
}

fn gib_s(bytes: usize, secs: f64) -> f64 {
    bytes as f64 / secs / (1 << 30) as f64
}

fn zstd_size(bytes: &[u8]) -> usize {
    zstd::bulk::compress(bytes, 3).unwrap().len()
}

// Every column of a Parquet file, concatenated across record batches.
fn parquet_columns(path: &str) -> Vec<(String, Vec<ArrayRef>)> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path).unwrap())
        .unwrap()
        .build()
        .unwrap();
    let mut columns: Vec<(String, Vec<ArrayRef>)> = Vec::new();
    for batch in reader {
        let batch = batch.unwrap();
        if columns.is_empty() {
            columns = batch
                .schema()
                .fields()
                .iter()
                .map(|f| (f.name().clone(), Vec::new()))
                .collect();
        }
        for (column, array) in columns.iter_mut().zip(batch.columns()) {
            column.1.push(array.clone());
        }
    }
    columns
}

// Totals across columns, for the per-file summary line.
#[derive(Default)]
struct Totals {
    naive: usize,
    encoded: usize,
    zstd: usize,
    decode_secs: f64,
}

impl Totals {
    fn add(&mut self, naive: usize, encoded: usize, zstd: usize, decode_secs: f64) {
        self.naive += naive;
        self.encoded += encoded;
        self.zstd += zstd;
        self.decode_secs += decode_secs;
    }

    fn print(&self, path: &str, what: &str) {
        println!(
            "{path}: all {what}: {} bytes, {:.2}x encoded, {:.2}x zstd -3, decode {:.1} GiB/s",
            self.naive,
            self.naive as f64 / self.encoded as f64,
            self.naive as f64 / self.zstd as f64,
            gib_s(self.naive, self.decode_secs)
        );
    }
}

fn row(name: &str, kind: &str, n: usize, naive: usize, encoded: usize, zstd: usize, secs: f64) {
    println!(
        "  {name:<28} {kind:<10} {n:>10} values {:>6.1} bits/value {:>6.2}x  zstd {:>6.2}x  decode {:>5.1} GiB/s",
        encoded as f64 * 8.0 / n as f64,
        naive as f64 / encoded as f64,
        naive as f64 / zstd as f64,
        gib_s(naive, secs)
    );
}

// Floats: per 1024-value vector, classic ALP (exponents searched per vector)
// cascaded into frame of reference + bitpacking, with exceptions as a 2-byte
// position plus the 8-byte float. For values that use the full mantissa,
// ALP-RD with one dictionary per column. Each column gets whichever is
// smaller, as Vortex's compressor does.
struct AlpVector {
    exponents: alp::Exponents,
    min: i64,
    width: usize,
    packed: Vec<u64>,
    patches: Vec<(u64, f64)>,
}

// Not alp::bit_width, which is at least 1: a constant vector packs to 0 bits.
fn for_width(encoded: &[i64]) -> usize {
    let min = *encoded.iter().min().unwrap();
    let max = *encoded.iter().max().unwrap();
    64 - (max.wrapping_sub(min) as u64).leading_zeros() as usize
}

// The exponents with the smallest encoded vector, out of every pair. The alp
// crate estimates from a 32-value sample, which misses rare outliers that are
// cheaper as exceptions than as a wider vector (e.g. the paper's Gov/26: 0.4
// bits per value exhaustively, 1.6 with the crate's choice).
fn best_exponents(values: &[f64]) -> alp::Exponents {
    let mut best = (usize::MAX, alp::Exponents { e: 0, f: 0 });
    for e in 0..=18 {
        for f in 0..=e {
            let exponents = alp::Exponents { e, f };
            let (_, encoded, positions, _, _) = alp::encode(values, Some(exponents));
            let size = CHUNK * for_width(&encoded) + positions.len() * 80;
            if size < best.0 {
                best = (size, exponents);
            }
        }
    }
    best.1
}

// Run f(i) for i in 0..n on all cores.
fn par_map<T: Send, F: Fn(usize) -> T + Sync>(n: usize, f: F) -> Vec<T> {
    let threads = std::thread::available_parallelism().unwrap().get();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut out: Vec<(usize, T)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if i >= n {
                            return mine;
                        }
                        mine.push((i, f(i)));
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    out.sort_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, t)| t).collect()
}

fn alp_vector(values: &[f64]) -> AlpVector {
    let (exponents, encoded, positions, patch_values, _) =
        alp::encode(values, Some(best_exponents(values)));
    let min = *encoded.iter().min().unwrap();
    let width = for_width(&encoded);
    let shifted: Vec<u64> = encoded.iter().map(|v| v.wrapping_sub(min) as u64).collect();
    let mut packed = vec![0u64; CHUNK * width / 64];
    unsafe { BitPacking::unchecked_pack(width, &shifted, &mut packed) };
    let patches = positions.into_iter().zip(patch_values).collect();
    AlpVector {
        exponents,
        min,
        width,
        packed,
        patches,
    }
}

fn alp_size(v: &AlpVector) -> usize {
    v.packed.len() * 8 + 8 + 2 + v.patches.len() * (2 + 8)
}

fn alp_decode(v: &AlpVector, out: &mut [i64; CHUNK]) {
    let out_u: &mut [u64] = unsafe { std::mem::transmute::<&mut [i64], &mut [u64]>(&mut out[..]) };
    unsafe { FoR::unchecked_unfor_pack(v.width, &v.packed, v.min as u64, out_u) };
    alp::decode_slice_inplace::<f64>(out, v.exponents);
    let floats: &mut [f64] = unsafe { std::mem::transmute::<&mut [i64], &mut [f64]>(&mut out[..]) };
    for &(position, value) in &v.patches {
        floats[position as usize] = value;
    }
}

struct RdVector {
    left_width: usize,
    right_width: u8,
    left: Vec<u16>,
    right: Vec<u64>,
    exceptions: (Vec<u64>, Vec<u16>),
}

fn rd_size(v: &RdVector) -> usize {
    v.left.len() * 2 + v.right.len() * 8 + v.exceptions.0.len() * (2 + 2)
}

// Pads a short last vector with its last value, so every vector is 1024
// values: packing a partial vector saves nothing.
fn vectors<T: Copy>(values: &[T]) -> Vec<Vec<T>> {
    values
        .chunks(CHUNK)
        .map(|c| {
            let mut v = c.to_vec();
            v.resize(CHUNK, *c.last().unwrap());
            v
        })
        .collect()
}

// Float64 columns of a Parquet file, or a raw little-endian f64 file (the
// format the ALP paper's datasets come in) as one column.
fn float_columns(path: &str) -> Vec<(String, Vec<f64>)> {
    if path.ends_with(".bin") {
        let name = std::path::Path::new(path).file_stem().unwrap();
        let bytes = std::fs::read(path).unwrap();
        let values = bytes
            .chunks_exact(8)
            .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
            .collect();
        return vec![(name.to_string_lossy().into_owned(), values)];
    }
    parquet_columns(path)
        .into_iter()
        .map(|(name, arrays)| {
            let values = arrays
                .iter()
                .filter_map(|a| a.as_primitive_opt::<Float64Type>())
                .flat_map(|a| a.iter().flatten())
                .collect();
            (name, values)
        })
        .collect()
}

fn floats(path: &str) {
    let mut totals = Totals::default();
    for (name, values) in float_columns(path) {
        if values.is_empty() {
            continue;
        }
        let n = values.len();
        let naive = n * 8;
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let zstd = zstd_size(&bytes);
        let chunks = vectors(&values);

        let alp: Vec<AlpVector> = par_map(chunks.len(), |i| alp_vector(&chunks[i]));
        let alp_bytes: usize = alp.iter().map(alp_size).sum();

        let sample: Vec<f64> = values.iter().step_by((n / 8192).max(1)).copied().collect();
        let encoder = alp::RDEncoder::new(&sample);
        let rd: Vec<RdVector> = chunks
            .iter()
            .map(|c| {
                let split = encoder.split(c);
                let left_width = split.left_parts_bit_width() as usize;
                let right_width = split.right_parts_bit_width();
                let mut left = vec![0u16; CHUNK * left_width / 16];
                let mut right = vec![0u64; CHUNK * right_width as usize / 64];
                unsafe {
                    BitPacking::unchecked_pack(left_width, split.left_parts(), &mut left);
                    BitPacking::unchecked_pack(
                        right_width as usize,
                        split.right_parts(),
                        &mut right,
                    );
                }
                let e = split.left_exceptions();
                RdVector {
                    left_width,
                    right_width,
                    left,
                    right,
                    exceptions: (e.positions().to_vec(), e.values().to_vec()),
                }
            })
            .collect();
        let rd_bytes: usize = rd.iter().map(rd_size).sum::<usize>() + 2 * encoder.codes().len();

        // Decode back to f64 and compare bits.
        let mut decoded = vec![0f64; chunks.len() * CHUNK];
        let (kind, encoded, secs) = if alp_bytes <= rd_bytes {
            let mut buf = [0i64; CHUNK];
            let secs = best_of(5, || {
                for (v, out) in alp.iter().zip(decoded.chunks_exact_mut(CHUNK)) {
                    alp_decode(v, &mut buf);
                    out.copy_from_slice(unsafe { std::mem::transmute::<&[i64], &[f64]>(&buf[..]) });
                }
            });
            ("ALP", alp_bytes, secs)
        } else {
            let dict = encoder.codes();
            let (mut left, mut right) = ([0u16; CHUNK], [0u64; CHUNK]);
            let secs = best_of(5, || {
                for (v, out) in rd.iter().zip(decoded.chunks_exact_mut(CHUNK)) {
                    unsafe {
                        BitPacking::unchecked_unpack(v.left_width, &v.left, &mut left);
                        BitPacking::unchecked_unpack(v.right_width as usize, &v.right, &mut right);
                    }
                    let floats = alp::alp_rd_decode::<f64>(
                        &left,
                        dict,
                        v.right_width,
                        &right,
                        &v.exceptions.0,
                        &v.exceptions.1,
                    );
                    out.copy_from_slice(&floats);
                }
            });
            ("ALP-RD", rd_bytes, secs)
        };
        assert!(
            values
                .iter()
                .zip(&decoded)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "{name} did not round-trip"
        );
        row(&name, kind, n, naive, encoded, zstd, secs);
        totals.add(naive, encoded, zstd, secs);
    }
    totals.print(path, "float columns");
}

// Integers: per 1024-value vector, frame of reference + bitpacking, or for
// columns where it is smaller, FastLanes delta (transposed into lanes, one
// base per lane) and then frame of reference + bitpacking over the deltas.
// Signed values are mapped to unsigned with their sign bit flipped, which
// keeps their order.
macro_rules! int_codec {
    ($name:ident, $t:ty, $lanes:literal) => {
        fn $name(values: &[$t]) -> (&'static str, usize, f64) {
            const BITS: usize = <$t>::BITS as usize;
            struct Packed {
                min: $t,
                width: usize,
                words: Vec<$t>,
                bases: [$t; $lanes],
            }
            fn pack(v: &[$t; CHUNK], bases: [$t; $lanes]) -> Packed {
                let min = *v.iter().min().unwrap();
                let max = *v.iter().max().unwrap();
                let width = (BITS - (max - min).leading_zeros() as usize);
                let shifted: Vec<$t> = v.iter().map(|x| x - min).collect();
                let mut words = vec![0 as $t; CHUNK * width / BITS];
                unsafe { BitPacking::unchecked_pack(width, &shifted, &mut words) };
                Packed {
                    min,
                    width,
                    words,
                    bases,
                }
            }
            let size = |p: &Packed| p.words.len() * BITS / 8 + BITS / 8;
            let chunks: Vec<[$t; CHUNK]> = vectors(values)
                .into_iter()
                .map(|c| c.try_into().unwrap())
                .collect();

            let for_packed: Vec<Packed> = chunks.iter().map(|c| pack(c, [0; $lanes])).collect();
            let for_bytes: usize = for_packed.iter().map(size).sum();

            let delta_packed: Vec<Packed> = chunks
                .iter()
                .map(|c| {
                    let mut transposed = [0 as $t; CHUNK];
                    Transpose::transpose(c, &mut transposed);
                    let bases: [$t; $lanes] = transposed[..$lanes].try_into().unwrap();
                    let mut deltas = [0 as $t; CHUNK];
                    Delta::delta::<$lanes>(&transposed, &bases, &mut deltas);
                    pack(&deltas, bases)
                })
                .collect();
            let delta_bytes: usize = delta_packed
                .iter()
                .map(|p| size(p) + $lanes * BITS / 8)
                .sum();

            let mut decoded = vec![0 as $t; chunks.len() * CHUNK];
            let use_delta = delta_bytes < for_bytes;
            let (mut tmp, mut tmp2) = ([0 as $t; CHUNK], [0 as $t; CHUNK]);
            let secs = best_of(5, || {
                if use_delta {
                    for (p, out) in delta_packed.iter().zip(decoded.chunks_exact_mut(CHUNK)) {
                        unsafe { FoR::unchecked_unfor_pack(p.width, &p.words, p.min, &mut tmp) };
                        Delta::undelta::<$lanes>(&tmp, &p.bases, &mut tmp2);
                        Transpose::untranspose(&tmp2, out.try_into().unwrap());
                    }
                } else {
                    for (p, out) in for_packed.iter().zip(decoded.chunks_exact_mut(CHUNK)) {
                        unsafe { FoR::unchecked_unfor_pack(p.width, &p.words, p.min, out) };
                    }
                }
            });
            assert_eq!(&decoded[..values.len()], values, "did not round-trip");
            if use_delta {
                ("delta", delta_bytes, secs)
            } else {
                ("FOR", for_bytes, secs)
            }
        }
    };
}

int_codec!(codec_u16, u16, 64);
int_codec!(codec_u32, u32, 32);
int_codec!(codec_u64, u64, 16);

fn ints(path: &str) {
    let mut totals = Totals::default();
    for (name, arrays) in parquet_columns(path) {
        // (values widened to u64 with the sign bit flipped, width in bytes)
        let mut values: Vec<u64> = Vec::new();
        let mut width = 0;
        for a in &arrays {
            macro_rules! take {
                ($t:ty, $w:expr, $flip:expr) => {
                    if let Some(a) = a.as_primitive_opt::<$t>() {
                        width = $w;
                        values.extend(a.iter().flatten().map(|v| (v as i64 as u64) ^ $flip));
                        continue;
                    }
                };
            }
            take!(Int16Type, 2, 1 << 15);
            take!(UInt16Type, 2, 0);
            take!(Int32Type, 4, 1 << 31);
            take!(Date32Type, 4, 1 << 31);
            take!(Int64Type, 8, 1 << 63);
            take!(TimestampMicrosecondType, 8, 1 << 63);
            take!(TimestampMillisecondType, 8, 1 << 63);
            take!(TimestampNanosecondType, 8, 1 << 63);
            take!(TimestampSecondType, 8, 1 << 63);
        }
        if values.is_empty() {
            continue;
        }
        let n = values.len();
        let naive = n * width;
        let (bytes, (kind, encoded, secs)): (Vec<u8>, _) = match width {
            2 => {
                let v: Vec<u16> = values.iter().map(|&x| x as u16).collect();
                (
                    v.iter().flat_map(|x| x.to_le_bytes()).collect(),
                    codec_u16(&v),
                )
            }
            4 => {
                let v: Vec<u32> = values.iter().map(|&x| x as u32).collect();
                (
                    v.iter().flat_map(|x| x.to_le_bytes()).collect(),
                    codec_u32(&v),
                )
            }
            _ => (
                values.iter().flat_map(|x| x.to_le_bytes()).collect(),
                codec_u64(&values),
            ),
        };
        let zstd = zstd_size(&bytes);
        row(&name, kind, n, naive, encoded, zstd, secs);
        totals.add(naive, encoded, zstd, secs);
    }
    totals.print(path, "integer columns");
}

// Strings: FSST with one symbol table per block of 64 Ki strings, trained on
// FSST's own 16 KiB sample of the block. Each string's compressed length is
// kept and bitpacked, so every string can still be decoded on its own.
fn fsst_column(name: &str, strings: &[&[u8]], totals: &mut Totals) {
    let n = strings.len();
    let data: usize = strings.iter().map(|s| s.len()).sum();
    let naive = data + 4 * n;
    let zstd = zstd_size(&strings.concat()) + 4 * n;
    let blocks: Vec<(fsst::Compressor, Vec<Vec<u8>>)> = strings
        .chunks(64 << 10)
        .map(|block| {
            let block = block.to_vec();
            let compressor = fsst::Compressor::train(&block);
            let compressed = compressor.compress_bulk(&block);
            (compressor, compressed)
        })
        .collect();
    let lengths: Vec<u32> = blocks
        .iter()
        .flat_map(|(_, c)| c.iter().map(|s| s.len() as u32))
        .collect();
    let codes: usize = lengths.iter().map(|&l| l as usize).sum();
    let (_, lengths_bytes, _) = codec_u32(&lengths);
    let tables: usize = blocks.iter().map(|(c, _)| c.n_symbols() * 9).sum();
    let encoded = codes + lengths_bytes + tables;

    let mut decoded = Vec::with_capacity(data + 8);
    let secs = best_of(5, || {
        decoded.clear();
        for (compressor, compressed) in &blocks {
            let decompressor = compressor.decompressor();
            for s in compressed {
                let start = decoded.len();
                decoded.reserve(decompressor.max_decompression_capacity(s));
                let spare = decoded.spare_capacity_mut();
                let len = decompressor.decompress_into(s, spare);
                unsafe { decoded.set_len(start + len) };
            }
        }
    });
    assert!(decoded == strings.concat(), "{name} did not round-trip");
    row(name, "FSST", n, naive, encoded, zstd, secs);
    totals.add(naive, encoded, zstd, secs);
}

fn strings(path: &str) {
    let mut totals = Totals::default();
    if path.ends_with(".parquet") {
        for (name, arrays) in parquet_columns(path) {
            let mut strings: Vec<&[u8]> = Vec::new();
            for a in &arrays {
                if let Some(a) = a.as_binary_opt::<i32>() {
                    strings.extend(a.iter().flatten());
                } else if let Some(a) = a.as_string_opt::<i32>() {
                    strings.extend(a.iter().flatten().map(str::as_bytes));
                }
            }
            // Skip mostly-empty columns: there is nothing to compress.
            if strings.is_empty()
                || strings.iter().map(|s| s.len()).sum::<usize>() < 4 * strings.len()
            {
                continue;
            }
            fsst_column(&name, &strings, &mut totals);
        }
    } else {
        // One string per line.
        let text = std::fs::read(path).unwrap();
        let lines: Vec<&[u8]> = text.split(|&b| b == b'\n').collect();
        fsst_column("lines", &lines, &mut totals);
    }
    totals.print(path, "string columns");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--bench").collect();
    let usage = "usage: column_codecs floats|ints|strings <files...>";
    // `cargo bench --bench <name>` adds --bench to the arguments; without any
    // other arguments there is nothing to measure, so skip quietly.
    if args.is_empty() {
        eprintln!("{usage} (no arguments, skipping)");
        return;
    }
    let (mode, files) = args.split_first().expect(usage);
    for f in files {
        match mode.as_str() {
            "floats" => floats(f),
            "ints" => ints(f),
            "strings" => strings(f),
            _ => panic!("{usage}"),
        }
    }
}
