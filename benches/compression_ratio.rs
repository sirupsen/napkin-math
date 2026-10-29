// Compression ratios for structured data: columnar tables, inverted indexes
// and time series. Ratios are against the naive in-memory representation
// (Arrow buffers, 4 bytes per doc ID, 16 bytes per point), except whole
// inverted and n-gram indexes, which are against their source text.
//
//   cargo bench --features encodings --bench compression_ratio -- parquet hits_0.parquet
//   cargo bench --features encodings --bench compression_ratio -- cardinality hits_0.parquet
//   cargo bench --features encodings --bench compression_ratio -- postings collection.tsv
//   cargo bench --features encodings --bench compression_ratio -- postings --paragraphs dickens
//   cargo bench --features encodings --bench compression_ratio -- timeseries nab/*.csv
use std::collections::HashMap;
use std::fs::File;
use std::time::Instant;

use arrow_array::cast::AsArray;
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
    // Single-threaded encode from and decode back to Arrow, both reported
    // against the Arrow size.
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
        let mut file = bytes::Bytes::new();
        let secs = best_of(3, || file = write(props.clone()));
        report(name, naive, file.len());
        println!(
            "    encode {:.2} GiB/s, {}, 1 thread",
            naive as f64 / secs / (1 << 30) as f64,
            decode(&file)
        );
    }
}

// Checks two napkin rules against Parquet's dictionary + RLE-hybrid encoding,
// uncompressed, one column at a time with one dictionary per column: a column
// with d distinct values costs log2(d) bits per value plus its dictionary, and
// a column in r runs costs about 3 bytes per run plus its dictionary,
// whichever is smaller.
fn cardinality(path: &str) {
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path).unwrap())
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<RecordBatch> = reader.map(|b| b.unwrap()).collect();
    let schema = batches[0].schema();
    let props = WriterProperties::builder()
        .set_compression(Compression::UNCOMPRESSED)
        .set_dictionary_page_size_limit(1 << 30)
        .set_max_row_group_row_count(None)
        .set_max_row_group_bytes(None)
        .build();
    println!(
        "  {:<24} {:>8} {:>9} {:>9} {:>8} {:>8} {:>8} {:>8}",
        "column", "rows", "distinct", "runs", "actual", "dict", "runs", "error"
    );
    for (c, field) in schema.fields().iter().enumerate() {
        // Each non-null value as bytes, to count distinct values and runs.
        let mut distinct = std::collections::HashSet::new();
        let (mut n, mut runs, mut dictionary) = (0usize, 0usize, 0usize);
        let mut prev: Option<Vec<u8>> = None;
        for batch in &batches {
            let a = batch.column(c);
            let data = a.to_data();
            let width = a.data_type().primitive_width();
            for i in (0..a.len()).filter(|&i| a.is_valid(i)) {
                let value: &[u8] = if let Some(w) = width {
                    let o = (a.offset() + i) * w;
                    &data.buffers()[0].as_slice()[o..o + w]
                } else if let Some(b) = a.as_binary_opt::<i32>() {
                    b.value(i)
                } else if let Some(s) = a.as_string_opt::<i32>() {
                    s.value(i).as_bytes()
                } else {
                    continue;
                };
                n += 1;
                if prev.as_deref() != Some(value) {
                    runs += 1;
                    prev = Some(value.to_vec());
                }
                if distinct.insert(value.to_vec()) {
                    // Parquet's plain dictionary page: the value, plus a 4-byte
                    // length for strings.
                    dictionary += value.len() + if width.is_some() { 0 } else { 4 };
                }
            }
        }
        if n == 0 {
            continue;
        }
        let mut out = Vec::new();
        let one = batches[0].project(&[c]).unwrap().schema();
        let mut writer = ArrowWriter::try_new(&mut out, one, Some(props.clone())).unwrap();
        for batch in &batches {
            writer.write(&batch.project(&[c]).unwrap()).unwrap();
        }
        writer.close().unwrap();
        let file = ParquetRecordBatchReaderBuilder::try_new(bytes::Bytes::from(out)).unwrap();
        let actual: i64 = file
            .metadata()
            .row_groups()
            .iter()
            .map(|g| g.column(0).uncompressed_size())
            .sum();
        let bits = |bytes: f64| bytes * 8.0 / n as f64;
        let log2 = (usize::BITS - (distinct.len() - 1).leading_zeros()) as f64;
        let by_dict = bits(dictionary as f64) + log2;
        let by_runs = bits((dictionary + 3 * runs) as f64);
        println!(
            "  {:<24} {:>8} {:>9} {:>9} {:>8.2} {:>8.2} {:>8.2} {:>7.0}%",
            field.name(),
            n,
            distinct.len(),
            runs,
            bits(actual as f64),
            by_dict,
            by_runs,
            100.0 * (bits(actual as f64) / by_dict.min(by_runs) - 1.0)
        );
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

// One n-gram's doc ID list, sized as it streams in (the lists for a large
// corpus don't fit in memory), in the same layout as bitpacked_size.
#[derive(Default)]
struct GramList {
    // Last doc ID plus one, 0 before the first.
    last: u32,
    block_len: u32,
    block_bits: u32,
    // Varint bytes of the current block, used if it never fills up.
    block_varint: usize,
    bytes: usize,
}

impl GramList {
    // Adds document `id`; returns whether it is a new posting.
    fn add(&mut self, id: u32) -> bool {
        if self.last == id {
            return false;
        }
        let delta = id - self.last;
        self.last = id;
        self.block_len += 1;
        self.block_bits = self.block_bits.max(32 - delta.leading_zeros());
        self.block_varint += varint(delta);
        if self.block_len as usize == BitPacker4x::BLOCK_LEN {
            self.bytes += 1 + BitPacker4x::compressed_block_size(self.block_bits as u8);
            (self.block_len, self.block_bits, self.block_varint) = (0, 0, 0);
        }
        true
    }
}

// A regex index in the style of Google Code Search: for every byte n-gram
// (ASCII-lowercased), the documents that contain it, with no frequencies or
// positions. Returns distinct n-grams, postings and index bytes, counting
// n bytes plus a 4-byte offset per n-gram for the dictionary.
fn ngram_index(documents: &[&str], n: usize) -> (usize, usize, usize) {
    gram_index(documents, n + 4, |text, emit| {
        let body: Vec<u8> = text.bytes().map(|b| b.to_ascii_lowercase()).collect();
        for gram in body.windows(n) {
            emit(gram.iter().fold(0, |k, &b| k << 8 | b as u64));
        }
    })
}

// GitHub code search's sparse n-grams, from its `sparse-ngrams` crate: every
// bigram, plus each 3-8 byte substring whose two end bigrams are rarer (in
// GitHub's code corpus) than every bigram between them, over text folded to
// one byte per character with `casefold`. A query looks up only the few
// grams that cover it, so it reads far fewer and more selective lists than
// with trigrams. Grams are opaque 32-bit keys, so the dictionary costs 4 bytes
// plus a 4-byte offset per gram.
fn sparse_index(documents: &[&str]) -> (usize, usize, usize) {
    gram_index(documents, 4 + 4, |text, emit| {
        let body = casefold::index_fold(text.to_string());
        sparse_ngrams::collect_sparse_grams_scan(&body, |gram, _| emit(gram.as_u32() as u64));
    })
}

// Builds a doc ID index over the gram keys `grams` emits for each document,
// with `entry` dictionary bytes per distinct gram.
fn gram_index<F>(documents: &[&str], entry: usize, mut grams: F) -> (usize, usize, usize)
where
    F: FnMut(&str, &mut dyn FnMut(u64)),
{
    let mut lists: HashMap<u64, GramList> = HashMap::new();
    let mut postings = 0;
    for (doc, text) in documents.iter().enumerate() {
        let id = doc as u32 + 1;
        grams(text, &mut |key| {
            postings += lists.entry(key).or_default().add(id) as usize;
        });
    }
    let size = lists
        .values()
        .map(|l| entry + l.bytes + l.block_varint)
        .sum();
    (lists.len(), postings, size)
}

// One document per line (MS MARCO's "<id>\t<text>", log lines, column values)
// or, with `paragraphs`, per blank-line-separated paragraph (prose). Doc IDs
// are document numbers, terms are lowercased alphanumeric runs.
fn postings(path: &str, paragraphs: bool) {
    let raw = std::fs::read(path).unwrap();
    let text = String::from_utf8_lossy(&raw).replace("\r\n", "\n");
    let mut index: HashMap<String, Postings> = HashMap::new();
    let mut word = String::new();
    let mut docs = 0u32;
    let documents: Vec<&str> = if paragraphs {
        text.split("\n\n")
            .filter(|p| !p.trim().is_empty())
            .collect()
    } else {
        text.lines().collect()
    };
    let documents: Vec<&str> = documents
        .into_iter()
        .map(|d| d.split_once('\t').map_or(d, |(_, b)| b))
        .collect();
    for &body in &documents {
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

    // The whole index against the source text. The term dictionary is
    // uncompressed: each term's bytes, a length byte and a 4-byte offset
    // into the postings.
    let dictionary: usize = index.keys().map(|t| t.len() + 1 + 4).sum();
    let without_positions = dictionary + doc_bitpacked + freq_bitpacked;
    println!("whole index, against {} bytes of source text:", raw.len());
    report(
        "dictionary + doc IDs + frequencies",
        raw.len(),
        without_positions,
    );
    report(
        "  + positions",
        raw.len(),
        without_positions + pos_bitpacked,
    );
    // Per document, and with documents merged into ~16 KiB chunks, about the
    // size of a source file, which is what code search indexes.
    let mut chunks = vec![String::new()];
    for d in &documents {
        if chunks.last().unwrap().len() >= 16 << 10 {
            chunks.push(String::new());
        }
        let chunk = chunks.last_mut().unwrap();
        chunk.push_str(d);
        chunk.push('\n');
    }
    let chunks: Vec<&str> = chunks.iter().map(String::as_str).collect();
    for (unit, docs) in [("document", &documents), ("16 KiB chunk", &chunks)] {
        for n in [3, 4] {
            let (grams, postings, size) = ngram_index(docs, n);
            report(
                &format!("{n}-grams per {unit} ({grams} grams, {postings} postings)"),
                raw.len(),
                size,
            );
        }
        let (grams, postings, size) = sparse_index(docs);
        report(
            &format!("sparse n-grams per {unit} ({grams} grams, {postings} postings)"),
            raw.len(),
            size,
        );
    }

    // Encode and decode speed: every full 128-ID block of every doc ID list,
    // packed contiguously, then unpacked, single-threaded.
    let packer = BitPacker4x::new();
    let mut blocks = Vec::new();
    let mut packed_len = 0;
    for p in index.values() {
        let mut prev = 0;
        for block in p.docs.chunks_exact(BitPacker4x::BLOCK_LEN) {
            let bits = packer.num_bits_sorted(prev, block);
            blocks.push((prev, packed_len, bits, block));
            packed_len += BitPacker4x::compressed_block_size(bits);
            prev = *block.last().unwrap();
        }
    }
    let mut packed = vec![0u8; packed_len];
    let encode_secs = best_of(5, || {
        let mut offset = 0;
        for &(initial, _, _, block) in &blocks {
            let bits = packer.num_bits_sorted(initial, block);
            offset += packer.compress_sorted(initial, block, &mut packed[offset..], bits);
        }
        std::hint::black_box(&packed);
    });
    let mut out = [0u32; BitPacker4x::BLOCK_LEN];
    let mut checksum = 0u64;
    let secs = best_of(5, || {
        for &(initial, offset, bits, _) in &blocks {
            packer.decompress_sorted(initial, &packed[offset..], &mut out, bits);
            checksum = checksum.wrapping_add(out[127] as u64);
        }
    });
    let ids = blocks.len() * BitPacker4x::BLOCK_LEN;
    println!(
        "encode bitpacked doc IDs, 1 thread: {:.0} M IDs/s, {:.1} GiB/s of u32 input",
        ids as f64 / encode_secs / 1e6,
        (ids * 4) as f64 / encode_secs / (1 << 30) as f64
    );
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

fn main() {
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != "--bench")
        .collect();
    let usage = "usage: compression_ratio parquet|cardinality|postings|timeseries <files...>";
    // `cargo bench --bench <name>` adds --bench to the arguments; without any
    // other arguments there is nothing to measure, so skip quietly.
    if args.is_empty() {
        eprintln!("{usage} (no arguments, skipping)");
        return;
    }
    let (mode, files) = args.split_first().expect(usage);
    match mode.as_str() {
        "parquet" => files.iter().for_each(|f| parquet(f)),
        "cardinality" => files.iter().for_each(|f| cardinality(f)),
        "postings" => {
            let paragraphs = files.first().is_some_and(|f| f == "--paragraphs");
            let files = &files[paragraphs as usize..];
            files.iter().for_each(|f| postings(f, paragraphs))
        }
        "timeseries" => timeseries(files),
        _ => panic!("{usage}"),
    }
}
