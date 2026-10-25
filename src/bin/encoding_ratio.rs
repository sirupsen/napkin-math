// Compression ratios for structured data: columnar tables, inverted indexes,
// time series and embeddings. Every ratio is against the naive in-memory
// representation (Arrow buffers, 4 bytes per doc ID, 16 bytes per point, 4
// bytes per dimension), not against a text export.
//
//   cargo run --release --features encodings --bin encoding_ratio -- parquet hits_0.parquet
//   cargo run --release --features encodings --bin encoding_ratio -- postings collection.tsv
//   cargo run --release --features encodings --bin encoding_ratio -- timeseries nab/*.csv
//   cargo run --release --features encodings --bin encoding_ratio -- vectors dbpedia/*.parquet
use std::collections::HashMap;
use std::fs::File;
use std::time::Instant;

use arrow_array::{Array, RecordBatch};
use bitpacking::{BitPacker, BitPacker4x};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

fn report(name: &str, naive: usize, size: usize) {
    println!(
        "  {:<34} {:>12} bytes  {:>6.1}x",
        name,
        size,
        naive as f64 / size as f64
    );
}

fn parquet(path: &str) {
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path).unwrap())
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<RecordBatch> = reader.map(|b| b.unwrap()).collect();
    let rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    let naive: usize = batches
        .iter()
        .flat_map(|b| b.columns().iter().map(|c| c.get_buffer_memory_size()))
        .sum();
    let schema = batches[0].schema();
    println!(
        "{path}: {rows} rows, {} columns, {naive} bytes in Arrow",
        schema.fields().len()
    );

    let write = |props: WriterProperties| {
        let mut out = Vec::new();
        let mut writer = ArrowWriter::try_new(&mut out, schema.clone(), Some(props)).unwrap();
        for batch in &batches {
            writer.write(batch).unwrap();
        }
        writer.close().unwrap();
        bytes::Bytes::from(out)
    };
    // Single-threaded decode back to Arrow, reported against the Arrow size.
    let decode = |file: &bytes::Bytes| {
        let secs = best_of(3, || {
            let reader = ParquetRecordBatchReaderBuilder::try_new(file.clone())
                .unwrap()
                .build()
                .unwrap();
            for batch in reader {
                std::hint::black_box(batch.unwrap());
            }
        });
        format!("decode {:.2} GiB/s", naive as f64 / secs / (1 << 30) as f64)
    };
    let plain = WriterProperties::builder()
        .set_dictionary_enabled(false)
        .set_compression(Compression::UNCOMPRESSED)
        .build();
    let encoded = WriterProperties::builder()
        .set_compression(Compression::UNCOMPRESSED)
        .build();
    let zstd = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(3).unwrap()))
        .build();
    for (name, props) in [
        ("parquet plain", plain),
        ("parquet dictionary + RLE", encoded),
        ("parquet dictionary + RLE + zstd -3", zstd),
    ] {
        let file = write(props);
        report(name, naive, file.len());
        println!("    {}, 1 thread", decode(&file));
    }
}

fn varint(mut v: u32) -> usize {
    let mut n = 1;
    while v >= 0x80 {
        v >>= 7;
        n += 1;
    }
    n
}

// Bytes for a stream of small integers in Lucene's layout: 128-value blocks of
// one bit-width byte plus the packed values, and a delta + varint tail.
fn bitpacked_size(values: &[u32]) -> usize {
    let packer = BitPacker4x::new();
    let mut blocks = values.chunks_exact(BitPacker4x::BLOCK_LEN);
    let mut size = 0;
    for block in &mut blocks {
        size += 1 + BitPacker4x::compressed_block_size(packer.num_bits(block));
    }
    size + blocks.remainder().iter().map(|&v| varint(v)).sum::<usize>()
}

fn best_of<F: FnMut()>(runs: usize, mut f: F) -> f64 {
    (0..runs)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed().as_secs_f64()
        })
        .fold(f64::MAX, f64::min)
}

#[derive(Default)]
struct Postings {
    docs: Vec<u32>,
    freqs: Vec<u32>,
    // Position deltas within each document, starting over at every document.
    positions: Vec<u32>,
    last_position: u32,
}

// MS MARCO passages: one "<id>\t<text>" line per document. Doc IDs are line
// numbers, terms are lowercased alphanumeric runs.
fn postings(path: &str) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut index: HashMap<String, Postings> = HashMap::new();
    let mut word = String::new();
    let mut docs = 0u32;
    for line in text.lines() {
        let body = line.split_once('\t').map_or(line, |(_, b)| b);
        for (position, w) in body
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .enumerate()
        {
            word.clear();
            word.extend(w.chars().flat_map(char::to_lowercase));
            let p = match index.get_mut(word.as_str()) {
                Some(p) => p,
                None => index.entry(word.clone()).or_default(),
            };
            let position = position as u32;
            if p.docs.last() == Some(&docs) {
                *p.freqs.last_mut().unwrap() += 1;
                p.positions.push(position - p.last_position);
            } else {
                p.docs.push(docs);
                p.freqs.push(1);
                p.positions.push(position);
            }
            p.last_position = position;
        }
        docs += 1;
    }
    let postings: usize = index.values().map(|p| p.docs.len()).sum();
    let positions: usize = index.values().map(|p| p.positions.len()).sum();
    println!(
        "{path}: {docs} documents, {} terms, {postings} postings, {positions} positions",
        index.len()
    );

    let (mut doc_bitpacked, mut doc_varint, mut doc_roaring) = (0, 0, 0);
    let (mut freq_bitpacked, mut pos_bitpacked) = (0, 0);
    let mut deltas = Vec::new();
    for p in index.values() {
        deltas.clear();
        let mut prev = 0;
        for &id in &p.docs {
            deltas.push(id - prev);
            prev = id;
        }
        doc_varint += deltas.iter().map(|&d| varint(d)).sum::<usize>();
        doc_bitpacked += bitpacked_size(&deltas);
        doc_roaring += roaring::RoaringBitmap::from_sorted_iter(p.docs.iter().copied())
            .unwrap()
            .serialized_size();
        let freqs: Vec<u32> = p.freqs.iter().map(|f| f - 1).collect();
        freq_bitpacked += bitpacked_size(&freqs);
        pos_bitpacked += bitpacked_size(&p.positions);
    }
    println!("doc IDs, against 4 B each:");
    report("delta + varint", postings * 4, doc_varint);
    report("delta + bitpacking", postings * 4, doc_bitpacked);
    report("roaring", postings * 4, doc_roaring);
    println!("term frequencies, against 4 B each:");
    report("bitpacking", postings * 4, freq_bitpacked);
    println!("positions, against 4 B each:");
    report("delta + bitpacking", positions * 4, pos_bitpacked);
    println!("doc IDs + frequencies + positions:");
    report(
        "bitpacking",
        (2 * postings + positions) * 4,
        doc_bitpacked + freq_bitpacked + pos_bitpacked,
    );

    // Decode speed: every full 128-ID block of every doc ID list, packed
    // contiguously, then unpacked single-threaded.
    let packer = BitPacker4x::new();
    let mut packed = Vec::new();
    let mut blocks = Vec::new();
    for p in index.values() {
        let mut prev = 0;
        for block in p.docs.chunks_exact(BitPacker4x::BLOCK_LEN) {
            let bits = packer.num_bits_sorted(prev, block);
            let offset = packed.len();
            packed.resize(offset + BitPacker4x::compressed_block_size(bits), 0);
            packer.compress_sorted(prev, block, &mut packed[offset..], bits);
            blocks.push((prev, offset, bits));
            prev = *block.last().unwrap();
        }
    }
    let mut out = [0u32; BitPacker4x::BLOCK_LEN];
    let mut checksum = 0u64;
    let secs = best_of(5, || {
        for &(initial, offset, bits) in &blocks {
            packer.decompress_sorted(initial, &packed[offset..], &mut out, bits);
            checksum = checksum.wrapping_add(out[127] as u64);
        }
    });
    let ids = blocks.len() * BitPacker4x::BLOCK_LEN;
    println!(
        "decode bitpacked doc IDs, 1 thread: {:.0} M IDs/s, {:.1} GiB/s of u32 output (checksum {checksum})",
        ids as f64 / secs / 1e6,
        (ids * 4) as f64 / secs / (1 << 30) as f64
    );
}

// Days since 1970-01-01 (Howard Hinnant's days_from_civil).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

// "YYYY-MM-DD HH:MM:SS,value" lines, as in the Numenta Anomaly Benchmark.
fn read_series(path: &str) -> Vec<(i64, f64)> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .skip(1)
        .map(|line| {
            let (ts, value) = line.split_once(',').unwrap();
            let n = |r: std::ops::Range<usize>| ts[r].parse::<i64>().unwrap();
            let secs = days_from_civil(n(0..4), n(5..7), n(8..10)) * 86400
                + n(11..13) * 3600
                + n(14..16) * 60
                + n(17..19);
            (secs, value.parse().unwrap())
        })
        .collect()
}

// Size in bits of the Gorilla encoding (Pelkonen et al., VLDB 2015):
// delta-of-delta timestamps and XORed float64 values.
fn gorilla_bits(points: &[(i64, f64)]) -> usize {
    let mut bits = 64 + 64;
    let (mut prev_ts, mut prev_delta) = (points[0].0, 0i64);
    let mut prev_value = points[0].1.to_bits();
    let (mut prev_leading, mut prev_trailing) = (u32::MAX, 0);
    for (i, &(ts, value)) in points.iter().enumerate().skip(1) {
        let delta = ts - prev_ts;
        if i == 1 {
            bits += 14;
        } else {
            bits += match delta - prev_delta {
                0 => 1,
                -63..=64 => 2 + 7,
                -255..=256 => 3 + 9,
                -2047..=2048 => 4 + 12,
                _ => 4 + 32,
            };
        }
        (prev_ts, prev_delta) = (ts, delta);

        let xor = value.to_bits() ^ prev_value;
        prev_value = value.to_bits();
        if xor == 0 {
            bits += 1;
            continue;
        }
        let leading = xor.leading_zeros().min(31);
        let trailing = xor.trailing_zeros();
        if prev_leading != u32::MAX && leading >= prev_leading && trailing >= prev_trailing {
            bits += 2 + (64 - prev_leading - prev_trailing) as usize;
        } else {
            bits += 2 + 5 + 6 + (64 - leading - trailing) as usize;
            (prev_leading, prev_trailing) = (leading, trailing);
        }
    }
    bits
}

fn timeseries(paths: &[String]) {
    let (mut total_naive, mut total_gorilla, mut total_zstd) = (0, 0, 0);
    for path in paths {
        let points = read_series(path);
        let naive = points.len() * 16;
        let gorilla = gorilla_bits(&points).div_ceil(8);
        // Column-wise: timestamp deltas, then values, zstd'd together.
        let mut raw = Vec::with_capacity(naive);
        let mut prev = 0;
        for &(ts, _) in &points {
            raw.extend((ts - prev).to_le_bytes());
            prev = ts;
        }
        for &(_, value) in &points {
            raw.extend(value.to_le_bytes());
        }
        let zstd = zstd::bulk::compress(&raw, 3).unwrap().len();
        println!(
            "{path}: {} points, gorilla {:.1}x ({:.1} B/point), delta + zstd -3 {:.1}x",
            points.len(),
            naive as f64 / gorilla as f64,
            gorilla as f64 / points.len() as f64,
            naive as f64 / zstd as f64
        );
        total_naive += naive;
        total_gorilla += gorilla;
        total_zstd += zstd;
    }
    println!("all series: {total_naive} bytes as 16 B/point");
    report("gorilla", total_naive, total_gorilla);
    report("delta + zstd -3", total_naive, total_zstd);
}

// Run f(i) for i in 0..n on all cores.
fn par_map<T: Send, F: Fn(usize) -> T + Sync>(n: usize, f: F) -> Vec<T> {
    let threads = std::thread::available_parallelism().unwrap().get();
    let chunk = n.div_ceil(threads);
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..n)
            .step_by(chunk.max(1))
            .map(|start| {
                let f = &f;
                s.spawn(move || (start..(start + chunk).min(n)).map(f).collect::<Vec<T>>())
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    })
}

fn top_k(scores: &[f32], k: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..scores.len()).collect();
    idx.select_nth_unstable_by(k, |&a, &b| scores[b].total_cmp(&scores[a]));
    idx.truncate(k);
    idx
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// Embedding columns: list<float> or list<double>, normalized to unit length.
fn read_vectors(paths: &[String]) -> (usize, Vec<f32>) {
    use arrow_array::cast::AsArray;
    use arrow_array::types::{Float32Type, Float64Type};
    use arrow_array::ArrayRef;
    let mut values = Vec::new();
    let mut dims = 0;
    for path in paths {
        let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path).unwrap())
            .unwrap()
            .build()
            .unwrap();
        for batch in reader {
            let batch = batch.unwrap();
            let schema = batch.schema();
            let (i, _) = schema
                .fields()
                .iter()
                .enumerate()
                .find(|(_, f)| f.name().contains("embedding"))
                .expect("no embedding column");
            let column = batch.column(i);
            let (inner, len): (&ArrayRef, usize) = match column.as_fixed_size_list_opt() {
                Some(list) => (list.values(), list.value_length() as usize),
                None => {
                    let list = column.as_list::<i32>();
                    (list.values(), list.value_length(0) as usize)
                }
            };
            dims = len;
            match inner.as_primitive_opt::<Float64Type>() {
                Some(f64s) => values.extend(f64s.values().iter().map(|&v| v as f32)),
                None => values.extend(inner.as_primitive::<Float32Type>().values().iter()),
            }
        }
    }
    for v in values.chunks_exact_mut(dims) {
        let norm = dot(v, v).sqrt();
        v.iter_mut().for_each(|x| *x /= norm);
    }
    (dims, values)
}

// Product quantization: m subspaces, 256 k-means centroids each, one byte per
// subspace.
struct Pq {
    sub: usize,
    centroids: Vec<Vec<f32>>, // per subspace, 256 * sub
}

fn nearest(centroids: &[f32], x: &[f32]) -> usize {
    centroids
        .chunks_exact(x.len())
        .map(|c| c.iter().zip(x).map(|(a, b)| (a - b) * (a - b)).sum::<f32>())
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
        .0
}

impl Pq {
    fn train(dims: usize, m: usize, train: &[f32]) -> Pq {
        let sub = dims / m;
        let n = train.len() / dims;
        let centroids = par_map(m, |s| {
            let point = |i: usize| &train[i * dims + s * sub..i * dims + (s + 1) * sub];
            let mut c: Vec<f32> = (0..256).flat_map(|i| point(i * n / 256).to_vec()).collect();
            for _ in 0..15 {
                let mut sums = vec![0f32; 256 * sub];
                let mut counts = vec![0usize; 256];
                for i in 0..n {
                    let j = nearest(&c, point(i));
                    counts[j] += 1;
                    sums[j * sub..(j + 1) * sub]
                        .iter_mut()
                        .zip(point(i))
                        .for_each(|(a, b)| *a += b);
                }
                for j in (0..256).filter(|&j| counts[j] > 0) {
                    for d in 0..sub {
                        c[j * sub + d] = sums[j * sub + d] / counts[j] as f32;
                    }
                }
            }
            c
        });
        Pq { sub, centroids }
    }

    fn encode(&self, v: &[f32]) -> Vec<u8> {
        self.centroids
            .iter()
            .zip(v.chunks_exact(self.sub))
            .map(|(c, x)| nearest(c, x) as u8)
            .collect()
    }

    // Asymmetric distance: inner products of the query with every centroid.
    fn scores(&self, q: &[f32], codes: &[u8]) -> Vec<f32> {
        let m = self.centroids.len();
        let table: Vec<f32> = self
            .centroids
            .iter()
            .zip(q.chunks_exact(self.sub))
            .flat_map(|(c, x)| c.chunks_exact(self.sub).map(move |c| dot(c, x)))
            .collect();
        codes
            .chunks_exact(m)
            .map(|code| {
                code.iter()
                    .enumerate()
                    .map(|(s, &j)| table[s * 256 + j as usize])
                    .sum()
            })
            .collect()
    }
}

fn vectors(paths: &[String]) {
    let (dims, values) = read_vectors(paths);
    let n = values.len() / dims;
    let raw: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    let naive = raw.len();
    println!("{n} vectors of {dims} dimensions, {naive} bytes as float32");
    // Byte-shuffle: all first bytes, then all second bytes, ... so the
    // exponent bytes sit together, like Blosc's shuffle filter.
    let mut shuffled = Vec::with_capacity(naive);
    for byte in 0..4 {
        shuffled.extend(raw.iter().skip(byte).step_by(4));
    }
    report(
        "zstd -3",
        naive,
        zstd::bulk::compress(&raw, 3).unwrap().len(),
    );
    report(
        "byte shuffle + zstd -3",
        naive,
        zstd::bulk::compress(&shuffled, 3).unwrap().len(),
    );

    // Lossy: recall@10 against exact inner-product search, no reranking,
    // 1000 held-out queries.
    const K: usize = 10;
    let queries = 1000;
    let (q, base) = values.split_at(queries * dims);
    let nb = base.len() / dims;
    fn row(data: &[f32], dims: usize, i: usize) -> &[f32] {
        &data[i * dims..(i + 1) * dims]
    }
    let exact = par_map(queries, |i| {
        let scores: Vec<f32> = (0..nb)
            .map(|j| dot(row(q, dims, i), row(base, dims, j)))
            .collect();
        top_k(&scores, K)
    });
    let recall = |scores: &(dyn Fn(usize) -> Vec<f32> + Sync)| {
        let found = par_map(queries, |i| {
            let approx = top_k(&scores(i), K);
            approx.iter().filter(|j| exact[i].contains(j)).count()
        });
        found.iter().sum::<usize>() as f64 / (queries * K) as f64
    };
    let lossy = |name: &str, bytes_per_vector: usize, r: f64| {
        println!(
            "  {:<34} {:>6.1}x  recall@10 {:.3}",
            name,
            (dims * 4) as f64 / bytes_per_vector as f64,
            r
        );
    };

    let f16: Vec<f32> = base
        .iter()
        .map(|&v| half::f16::from_f32(v).to_f32())
        .collect();
    lossy(
        "float16",
        dims * 2,
        recall(&|i| {
            (0..nb)
                .map(|j| dot(row(q, dims, i), row(&f16, dims, j)))
                .collect()
        }),
    );

    // Scalar quantization: per-dimension min/max to 0..255.
    let (mut lo, mut hi) = (vec![f32::MAX; dims], vec![f32::MIN; dims]);
    for v in base.chunks_exact(dims) {
        for d in 0..dims {
            lo[d] = lo[d].min(v[d]);
            hi[d] = hi[d].max(v[d]);
        }
    }
    let int8: Vec<f32> = base
        .chunks_exact(dims)
        .flat_map(|v| {
            (0..dims).map(|d| {
                let step = (hi[d] - lo[d]) / 255.0;
                lo[d] + ((v[d] - lo[d]) / step).round() * step
            })
        })
        .collect();
    lossy(
        "int8 (scalar)",
        dims,
        recall(&|i| {
            (0..nb)
                .map(|j| dot(row(q, dims, i), row(&int8, dims, j)))
                .collect()
        }),
    );

    // Binary: one sign bit per dimension, Hamming distance between query and
    // vector bits.
    fn bits(v: &[f32]) -> Vec<u64> {
        v.chunks(64)
            .map(|c| {
                c.iter()
                    .enumerate()
                    .fold(0u64, |acc, (b, &x)| acc | ((x > 0.0) as u64) << b)
            })
            .collect()
    }
    let base_bits: Vec<Vec<u64>> = base.chunks_exact(dims).map(bits).collect();
    let hamming = |i: usize| -> Vec<f32> {
        let qb = bits(row(q, dims, i));
        base_bits
            .iter()
            .map(|b| {
                -(b.iter()
                    .zip(&qb)
                    .map(|(x, y)| (x ^ y).count_ones())
                    .sum::<u32>() as f32)
            })
            .collect()
    };
    lossy("binary", dims / 8, recall(&hamming));
    // Binary, then rerank the top 100 with float32.
    let reranked = |i: usize| -> Vec<f32> {
        let candidates = top_k(&hamming(i), 10 * K);
        let mut scores = vec![f32::MIN; nb];
        for j in candidates {
            scores[j] = dot(row(q, dims, i), row(base, dims, j));
        }
        scores
    };
    lossy(
        "binary + float32 rerank of top 100",
        dims / 8,
        recall(&reranked),
    );

    let train = &base[..base.len().min(50_000 * dims)];
    for m in [dims / 8, dims / 32] {
        let pq = Pq::train(dims, m, train);
        let codes: Vec<u8> = par_map(nb, |j| pq.encode(row(base, dims, j))).concat();
        lossy(
            &format!("PQ, {m} bytes"),
            m,
            recall(&|i| pq.scores(row(q, dims, i), &codes)),
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: encoding_ratio parquet|postings|timeseries|vectors <files...>";
    let (mode, files) = args.split_first().expect(usage);
    match mode.as_str() {
        "parquet" => files.iter().for_each(|f| parquet(f)),
        "postings" => files.iter().for_each(|f| postings(f)),
        "timeseries" => timeseries(files),
        "vectors" => vectors(files),
        _ => panic!("{usage}"),
    }
}
