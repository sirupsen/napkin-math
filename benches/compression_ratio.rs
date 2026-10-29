// Compression ratios for structured data: columnar tables, inverted indexes
// and time series. Ratios are against the naive in-memory representation
// (Arrow buffers, 4 bytes per doc ID, 16 bytes per point), except whole
// inverted and n-gram indexes, which are against their source text.
//
//   cargo bench --features encodings --bench compression_ratio -- parquet hits_0.parquet
//   cargo bench --features encodings --bench compression_ratio -- cardinality hits_0.parquet
//   cargo bench --features encodings --bench compression_ratio -- postings collection.tsv
//   cargo bench --features encodings --bench compression_ratio -- postings --paragraphs dickens
//   cargo bench --features encodings --bench compression_ratio -- postings --files linux/
//   cargo bench --features encodings --bench compression_ratio -- timeseries nab/*.csv
//   cargo bench --features encodings --bench compression_ratio -- columns hits_0.parquet out/ Title URL
//   cargo bench --features encodings --bench compression_ratio -- index_sizes datasets/manifest.tsv
use rustc_hash::{FxHashMap, FxHashSet};
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

// Posting lists are sized in Tantivy's layout: full 128-value blocks bitpacked
// with BitPacker4x, the rest of a list as varints. Doc ID gaps are packed
// minus one (`compress_strictly_sorted`), so runs of consecutive documents
// cost 0 bits, and term frequencies minus one. Each full doc ID block has a
// skip entry: the last doc ID and its bit width, plus the frequencies' bit
// width and block-max score if there are frequencies, plus the block's
// frequency sum if there are positions.
const SKIP_DOCS: usize = 4 + 1;
const SKIP_FREQS: usize = 1 + 2;
const SKIP_POSITIONS: usize = 4;

fn bits(max: u32) -> u8 {
    (32 - max.leading_zeros()) as u8
}

// Packed bytes of `values`, each minus `minus` in full blocks, without the
// bit widths, and the number of full blocks.
fn packed_size(values: &[u32], minus: u32) -> (usize, usize) {
    let mut blocks = values.chunks_exact(BitPacker4x::BLOCK_LEN);
    let mut size = 0;
    let mut n = 0;
    for block in &mut blocks {
        let max = block.iter().map(|&v| v - minus).max().unwrap();
        size += BitPacker4x::compressed_block_size(bits(max));
        n += 1;
    }
    (size + blocks.remainder().iter().map(|&v| varint(v)).sum::<usize>(), n)
}

// The skip list for a term with `blocks` full doc ID blocks, `entry` bytes
// each, behind a varint length.
fn skip_size(blocks: usize, entry: usize) -> usize {
    if blocks == 0 {
        0
    } else {
        varint((blocks * entry) as u32) + blocks * entry
    }
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
// corpus don't fit in memory), in the same layout as packed_size, with no
// frequencies.
#[derive(Default)]
struct GramList {
    // Last doc ID plus one, 0 before the first.
    last: u32,
    block_len: u32,
    // Largest gap minus one in the current block.
    block_max: u32,
    // Varint bytes of the current block, used if it never fills up.
    block_varint: usize,
    blocks: usize,
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
        self.block_max = self.block_max.max(delta - 1);
        self.block_varint += varint(delta);
        if self.block_len as usize == BitPacker4x::BLOCK_LEN {
            self.bytes += BitPacker4x::compressed_block_size(bits(self.block_max));
            self.blocks += 1;
            (self.block_len, self.block_max, self.block_varint) = (0, 0, 0);
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
    let mut lists: FxHashMap<u64, GramList> = FxHashMap::default();
    let mut postings = 0;
    for (doc, text) in documents.iter().enumerate() {
        let id = doc as u32 + 1;
        grams(text, &mut |key| {
            postings += lists.entry(key).or_default().add(id) as usize;
        });
    }
    let size = lists
        .values()
        .map(|l| entry + l.bytes + l.block_varint + skip_size(l.blocks, SKIP_DOCS))
        .sum();
    (lists.len(), postings, size)
}

#[derive(Clone, Copy, PartialEq)]
enum Split {
    Lines,
    // Lines of "<id>\t<text>", indexing the text.
    Tsv,
    Paragraphs,
    Files,
}

// GitHub code search skips files over 350 KiB.
const MAX_FILE: u64 = 350 << 10;

// Every text file under `dir` (no NUL byte, at most MAX_FILE), skipping .git,
// in sorted order so doc IDs are reproducible.
fn source_files(dir: &std::path::Path, out: &mut Vec<String>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let kind = e.file_type().unwrap();
        if kind.is_dir() && e.file_name() != ".git" {
            source_files(&e.path(), out);
        } else if kind.is_file() && e.metadata().unwrap().len() <= MAX_FILE {
            let bytes = std::fs::read(e.path()).unwrap();
            if !bytes.contains(&0) {
                out.push(String::from_utf8_lossy(&bytes).into_owned());
            }
        }
    }
}

// Bits per key of a bloom filter with the optimal number of hashes at a 1%
// false-positive rate: -ln(0.01) / ln(2)^2.
const BLOOM_BITS: f64 = 9.585;

// Bytes of token and trigram bloom filters over blocks of `block` bytes of
// consecutive documents, with the same tokens as the inverted index and the
// same trigrams as ngram_index, and of the blocks themselves compressed with
// zstd -3, which a query decompresses and scans on every match.
fn bloom_size(documents: &[&str], block: usize) -> (usize, usize, usize) {
    let (mut tokens, mut trigrams) = (FxHashSet::default(), FxHashSet::default());
    let (mut token_bytes, mut trigram_bytes) = (0.0, 0.0);
    let (mut text, mut zstd_bytes) = (Vec::new(), 0);
    let mut flush = |tokens: &mut FxHashSet<u64>, trigrams: &mut FxHashSet<u32>, text: &mut Vec<u8>| {
        token_bytes += (tokens.len() as f64 * BLOOM_BITS / 8.0).ceil();
        trigram_bytes += (trigrams.len() as f64 * BLOOM_BITS / 8.0).ceil();
        zstd_bytes += zstd::bulk::compress(text, 3).unwrap().len();
        tokens.clear();
        trigrams.clear();
        text.clear();
    };
    for d in documents {
        for w in d.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
            tokens.insert(xxhash_rust::xxh3::xxh3_64(w.to_lowercase().as_bytes()));
        }
        for g in d.as_bytes().windows(3) {
            trigrams.insert(g.iter().fold(0, |k, b| k << 8 | b.to_ascii_lowercase() as u32));
        }
        text.extend_from_slice(d.as_bytes());
        text.push(b'\n');
        if text.len() >= block {
            flush(&mut tokens, &mut trigrams, &mut text);
        }
    }
    flush(&mut tokens, &mut trigrams, &mut text);
    (token_bytes as usize, trigram_bytes as usize, zstd_bytes)
}

fn line(name: &str, naive: usize, size: usize) -> String {
    format!(
        "  {:<34} {:>12} bytes  {:>6.1}x",
        name,
        size,
        naive as f64 / size as f64
    )
}

// The inverted index over `documents`: report lines and the whole index
// without and with positions, skip lists included. The term dictionary is
// uncompressed: each term's bytes, a length byte and a 4-byte offset into the
// postings. With `details`, also sizes doc IDs as delta + varint and as
// roaring bitmaps, and times packing and unpacking them.
fn inverted(documents: &[&str], raw_len: usize, details: bool) -> (Vec<String>, usize, usize) {
    let mut index: FxHashMap<String, Postings> = FxHashMap::default();
    let mut word = String::new();
    for (doc, &body) in documents.iter().enumerate() {
        let doc = doc as u32;
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
            if p.docs.last() == Some(&doc) {
                *p.freqs.last_mut().unwrap() += 1;
                p.positions.push(position - p.last_position);
            } else {
                p.docs.push(doc);
                p.freqs.push(1);
                p.positions.push(position);
            }
            p.last_position = position;
        }
    }
    let postings: usize = index.values().map(|p| p.docs.len()).sum();
    let positions: usize = index.values().map(|p| p.positions.len()).sum();
    let mut out = vec![format!(
        "{} documents, {} terms, {postings} postings, {positions} positions",
        documents.len(),
        index.len()
    )];

    let (mut doc_bitpacked, mut freq_bitpacked, mut pos_bitpacked) = (0, 0, 0);
    let (mut skip_docs, mut skip_freqs, mut skip_positions) = (0, 0, 0);
    let mut deltas = Vec::new();
    for p in index.values() {
        deltas.clear();
        let mut prev = 0;
        for &id in &p.docs {
            deltas.push(id - prev);
            prev = id;
        }
        let (doc_bytes, blocks) = packed_size(&deltas, 1);
        doc_bitpacked += doc_bytes;
        skip_docs += skip_size(blocks, SKIP_DOCS);
        skip_freqs += skip_size(blocks, SKIP_DOCS + SKIP_FREQS);
        skip_positions += skip_size(blocks, SKIP_DOCS + SKIP_FREQS + SKIP_POSITIONS);
        freq_bitpacked += packed_size(&p.freqs, 1).0;
        // Positions are one stream per term, with a bit width per block.
        let (pos_bytes, pos_blocks) = packed_size(&p.positions, 0);
        pos_bitpacked += varint(pos_blocks as u32) + pos_blocks + pos_bytes;
    }
    out.push("doc IDs, frequencies and positions, against 4 B each:".into());
    out.push(line("doc IDs", postings * 4, doc_bitpacked));
    out.push(line("  + skip entries", postings * 4, doc_bitpacked + skip_docs));
    out.push(line("frequencies", postings * 4, freq_bitpacked));
    out.push(line("positions", positions * 4, pos_bitpacked));

    let dictionary: usize = index.keys().map(|t| t.len() + 1 + 4).sum();
    let without_positions = dictionary + doc_bitpacked + freq_bitpacked + skip_freqs;
    let with_positions = dictionary + doc_bitpacked + freq_bitpacked + pos_bitpacked + skip_positions;
    out.push(format!("whole index, against {raw_len} bytes of source text:"));
    out.push(line("dictionary + doc IDs + frequencies", raw_len, without_positions));
    out.push(line("  + positions", raw_len, with_positions));
    if details {
        let varints: usize = index
            .values()
            .map(|p| {
                let gaps = p.docs.iter().scan(0, |prev, &id| Some(id - std::mem::replace(prev, id)));
                gaps.map(varint).sum::<usize>()
            })
            .sum();
        let roaring: usize = index
            .values()
            .map(|p| {
                roaring::RoaringBitmap::from_sorted_iter(p.docs.iter().copied())
                    .unwrap()
                    .serialized_size()
            })
            .sum();
        out.push(line("doc IDs as delta + varint", postings * 4, varints));
        out.push(line("doc IDs as roaring", postings * 4, roaring));
        out.extend(bitpacking_speed(&index));
    }
    (out, without_positions, with_positions)
}

// Encode and decode speed: every full 128-ID block of every doc ID list,
// packed contiguously, then unpacked, single-threaded.
fn bitpacking_speed(index: &FxHashMap<String, Postings>) -> Vec<String> {
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
    vec![
        format!(
            "encode bitpacked doc IDs, 1 thread: {:.0} M IDs/s, {:.1} GiB/s of u32 input",
            ids as f64 / encode_secs / 1e6,
            (ids * 4) as f64 / encode_secs / (1 << 30) as f64
        ),
        format!(
            "decode bitpacked doc IDs, 1 thread: {:.0} M IDs/s, {:.1} GiB/s of u32 output (checksum {checksum})",
            ids as f64 / secs / 1e6,
            (ids * 4) as f64 / secs / (1 << 30) as f64
        ),
    ]
}

// One document per line (log lines, column values; with --tsv, the text after
// the first tab, as in MS MARCO's "<id>\t<text>"),
// per blank-line-separated paragraph (prose), or per source file under a
// directory (code). Doc IDs are document numbers, terms are lowercased
// alphanumeric runs. The n-gram indexes are per source file for code, and per
// ~16 KiB chunk of consecutive documents otherwise; bloom filters per 1 MiB.
// The indexes build on separate threads.
fn postings(path: &str, split: Split, details: bool) {
    let (lines, _) = index_sizes(path, split, details);
    lines.iter().for_each(|l| println!("{l}"));
}

// What index_sizes measures, each index as a fraction of the text.
struct Sizes {
    documents: usize,
    bytes: usize,
    // Inverted, + positions, 3-grams, 4-grams, sparse n-grams, zstd -3, token
    // bloom, trigram bloom.
    fractions: [f64; 8],
}

fn index_sizes(path: &str, split: Split, details: bool) -> (Vec<String>, Sizes) {
    let mut files = Vec::new();
    let (raw_len, text) = if split == Split::Files {
        source_files(std::path::Path::new(path), &mut files);
        (files.iter().map(String::len).sum(), String::new())
    } else {
        let raw = std::fs::read(path).unwrap();
        (raw.len(), String::from_utf8_lossy(&raw).replace("\r\n", "\n"))
    };
    let documents: Vec<&str> = match split {
        Split::Paragraphs => text
            .split("\n\n")
            .filter(|p| !p.trim().is_empty())
            .collect(),
        Split::Lines => text.lines().collect(),
        Split::Tsv => text
            .lines()
            .map(|d| d.split_once('\t').map_or(d, |(_, b)| b))
            .collect(),
        Split::Files => files.iter().map(String::as_str).collect(),
    };
    let mut chunks = vec![String::new()];
    if split != Split::Files {
        for d in &documents {
            if chunks.last().unwrap().len() >= 16 << 10 {
                chunks.push(String::new());
            }
            let chunk = chunks.last_mut().unwrap();
            chunk.push_str(d);
            chunk.push('\n');
        }
    }
    let chunks: Vec<&str> = chunks.iter().map(String::as_str).collect();
    let (unit, gram_docs) = if split == Split::Files {
        ("file", &documents)
    } else {
        ("16 KiB chunk", &chunks)
    };

    let ((inv, without_positions, with_positions), tri, four, sparse, bloom) =
        std::thread::scope(|s| {
            let inv = s.spawn(|| inverted(&documents, raw_len, details));
            let tri = s.spawn(|| ngram_index(gram_docs, 3));
            let four = s.spawn(|| ngram_index(gram_docs, 4));
            let sparse = s.spawn(|| sparse_index(gram_docs));
            let bloom = s.spawn(|| bloom_size(&documents, 1 << 20));
            (
                inv.join().unwrap(),
                tri.join().unwrap(),
                four.join().unwrap(),
                sparse.join().unwrap(),
                bloom.join().unwrap(),
            )
        });
    let mut out = vec![format!("{path}: {}", inv[0])];
    out.extend(inv[1..].iter().cloned());
    for (name, (grams, postings, size)) in [("3-grams", tri), ("4-grams", four), ("sparse n-grams", sparse)] {
        let name = format!("{name} per {unit} ({grams} grams, {postings} postings)");
        out.push(line(&name, raw_len, size));
    }
    let (tokens, trigrams, blocks) = bloom;
    out.push(line("zstd -3 per 1 MiB", raw_len, blocks));
    out.push(line("token bloom per 1 MiB, 1% FP", raw_len, tokens));
    out.push(line("trigram bloom per 1 MiB, 1% FP", raw_len, trigrams));
    let sizes = [without_positions, with_positions, tri.2, four.2, sparse.2, blocks, tokens, trigrams];
    let fractions = sizes.map(|size| size as f64 / raw_len as f64);
    (out, Sizes { documents: documents.len(), bytes: raw_len, fractions })
}

// Every dataset in a manifest written by benches/index_datasets.sh, one per
// line as "kind<TAB>name<TAB>lines|tsv|paragraphs|files<TAB>path" (relative to the
// manifest), on `jobs` threads, largest first. Prints the per-dataset table
// and the median of each kind as Markdown.
fn index_manifest(manifest: &str, jobs: usize) {
    let dir = std::path::Path::new(manifest).parent().unwrap();
    let mut datasets: Vec<(String, String, Split, String)> = std::fs::read_to_string(manifest)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let split = match f[2] {
                "lines" => Split::Lines,
                "tsv" => Split::Tsv,
                "paragraphs" => Split::Paragraphs,
                "files" => Split::Files,
                other => panic!("unknown split {other}"),
            };
            let path = dir.join(f[3]).to_string_lossy().into_owned();
            (f[0].to_string(), f[1].to_string(), split, path)
        })
        .collect();
    fn size(path: &std::path::Path) -> u64 {
        let meta = std::fs::symlink_metadata(path).unwrap();
        if !meta.is_dir() {
            return meta.len();
        }
        std::fs::read_dir(path).unwrap().map(|e| size(&e.unwrap().path())).sum()
    }
    datasets.sort_by_key(|d| std::cmp::Reverse(size(std::path::Path::new(&d.3))));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some((kind, name, split, path)) = datasets.get(i) else { break };
                let start = Instant::now();
                let (_, sizes) = index_sizes(path, *split, false);
                eprintln!("{name}: {:.1} s", start.elapsed().as_secs_f64());
                results.lock().unwrap().push((kind.clone(), name.clone(), sizes));
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    let kinds: Vec<String> = {
        let mut k: Vec<String> = Vec::new();
        for (kind, ..) in &datasets {
            if !k.contains(kind) {
                k.push(kind.clone());
            }
        }
        k.sort_by_key(|k| ["Prose", "Code", "Logs", "Relational Columns"].iter().position(|x| x == k).unwrap_or(9));
        k
    };
    results.sort_by(|a, b| {
        let rank = |k: &str| kinds.iter().position(|x| x == k).unwrap();
        rank(&a.0).cmp(&rank(&b.0)).then(b.2.bytes.cmp(&a.2.bytes))
    });
    let header = "Inverted | + positions | 3-grams | 4-grams | Sparse | zstd -3 | Token bloom | Trigram bloom";
    let count = |n: usize| {
        if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1e6) } else { format!("{}k", (n as f64 / 1e3).round()) }
    };
    let bytes = |n: usize| {
        if n >= 1_000_000_000 { format!("{:.1} GB", n as f64 / 1e9) } else { format!("{} MB", (n as f64 / 1e6).round()) }
    };
    let cells = |f: &[f64; 8]| {
        f.iter()
            .enumerate()
            .map(|(i, v)| if i >= 5 { format!("{v:.3}") } else { format!("{v:.2}") })
            .collect::<Vec<_>>()
            .join(" | ")
    };
    println!("| Dataset | Kind | Docs | Text | {header} |");
    println!("|{}", " --- |".repeat(12));
    for (kind, name, s) in &results {
        println!("| {name} | {kind} | {} | {} | {} |", count(s.documents), bytes(s.bytes), cells(&s.fractions));
    }
    println!();
    println!("| Kind | {header} |");
    println!("|{}", " --- |".repeat(9));
    for kind in &kinds {
        let rows: Vec<&Sizes> = results.iter().filter(|r| &r.0 == kind).map(|r| &r.2).collect();
        let median = std::array::from_fn(|i| {
            let mut v: Vec<f64> = rows.iter().map(|r| r.fractions[i]).collect();
            v.sort_by(f64::total_cmp);
            let n = v.len();
            if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
        });
        println!("| {kind} ({}) | {} |", rows.len(), cells(&median));
    }
}

// Writes the non-empty values of string `columns` in a Parquet file to
// `<out_dir>/<column>.txt`, one per line with line breaks replaced by spaces,
// or, with `--books`, separated by blank lines with line breaks kept.
fn columns(args: &[String]) {
    let books = args.iter().any(|a| a == "--books");
    let args: Vec<&String> = args.iter().filter(|a| *a != "--books").collect();
    let (path, out_dir, names) = (args[0], args[1], &args[2..]);
    for name in names {
        let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path).unwrap())
            .unwrap();
        let mask = parquet::arrow::ProjectionMask::columns(reader.parquet_schema(), [name.as_str()]);
        let reader = reader.with_projection(mask).build().unwrap();
        let mut out = std::io::BufWriter::new(File::create(format!("{out_dir}/{name}.txt")).unwrap());
        for batch in reader {
            let batch = batch.unwrap();
            let column = batch.column(0);
            let value = |i: usize| -> Vec<u8> {
                if let Some(c) = column.as_string_opt::<i32>() {
                    c.value(i).as_bytes().to_vec()
                } else if let Some(c) = column.as_string_opt::<i64>() {
                    c.value(i).as_bytes().to_vec()
                } else if let Some(c) = column.as_binary_opt::<i32>() {
                    c.value(i).to_vec()
                } else if let Some(c) = column.as_string_view_opt() {
                    c.value(i).as_bytes().to_vec()
                } else if let Some(c) = column.as_binary_view_opt() {
                    c.value(i).to_vec()
                } else {
                    panic!("{name} is {}, not a string column", column.data_type())
                }
            };
            for i in 0..column.len() {
                if column.is_null(i) {
                    continue;
                }
                let mut v = value(i);
                if v.is_empty() {
                    continue;
                }
                if books {
                    v.extend_from_slice(b"\n\n");
                } else {
                    v.iter_mut().filter(|b| **b == b'\n' || **b == b'\r').for_each(|b| *b = b' ');
                    v.push(b'\n');
                }
                std::io::Write::write_all(&mut out, &v).unwrap();
            }
        }
    }
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
    let usage = "usage: compression_ratio parquet|cardinality|postings|timeseries|columns|index_sizes <files...>";
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
            let details = files.iter().any(|f| f == "--details");
            let files: Vec<&String> = files.iter().filter(|f| *f != "--details").collect();
            let split = match files.first().map(|f| f.as_str()) {
                Some("--paragraphs") => Split::Paragraphs,
                Some("--files") => Split::Files,
                Some("--tsv") => Split::Tsv,
                _ => Split::Lines,
            };
            let files = &files[(split != Split::Lines) as usize..];
            files.iter().for_each(|f| postings(f, split, details))
        }
        "timeseries" => timeseries(files),
        "columns" => columns(files),
        "index_sizes" => {
            let jobs = std::thread::available_parallelism().map_or(4, |n| n.get() / 4).max(1);
            index_manifest(&files[0], jobs)
        }
        _ => panic!("{usage}"),
    }
}
