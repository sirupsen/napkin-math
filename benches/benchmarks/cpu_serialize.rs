use super::MIB;
use criterion::*;
use prost::Message;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use serde::{Deserialize, Serialize};
use std::hint::black_box;

#[derive(Clone, PartialEq, Serialize, Deserialize, Message)]
pub struct BenchPayload {
    #[prost(message, repeated, tag = "1")]
    pub events: Vec<BenchEvent>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Message)]
pub struct BenchEvent {
    #[prost(uint64, tag = "1")]
    pub event_id: u64,
    #[prost(uint64, tag = "2")]
    pub user_id: u64,
    #[prost(uint32, tag = "3")]
    pub shard: u32,
    #[prost(uint64, tag = "4")]
    pub timestamp_ms: u64,
    #[prost(uint64, tag = "5")]
    pub amount_cents: u64,
    #[prost(string, tag = "6")]
    pub region: String,
    #[prost(string, tag = "7")]
    pub route: String,
    #[prost(string, tag = "8")]
    pub sku: String,
    #[prost(string, tag = "9")]
    pub note: String,
    #[prost(string, repeated, tag = "10")]
    pub tags: Vec<String>,
    #[prost(bool, tag = "11")]
    pub success: bool,
}

pub struct SerializationCorpus {
    pub payload: BenchPayload,
    pub protobuf: Vec<u8>,
    pub json: Vec<u8>,
}

pub fn serialization_corpus() -> SerializationCorpus {
    const TARGET_PROTO_BYTES: usize = MIB;
    let mut rng = StdRng::seed_from_u64(0x5eed_5eed);
    let mut payload = BenchPayload { events: Vec::new() };

    let regions = ["us-east1", "us-central1", "europe-west1", "ap-southeast1"];
    let routes = [
        "/api/search",
        "/api/cart/add",
        "/api/checkout",
        "/api/recommendations",
    ];
    let skus = [
        "napkin-classic",
        "napkin-premium",
        "coaster-pack",
        "tablecloth-small",
        "tablecloth-large",
        "glass-set",
    ];
    let tag_pool = [
        "organic",
        "sale",
        "priority",
        "new",
        "gift",
        "popular",
        "subscription",
        "limited",
    ];

    while payload.encoded_len() < TARGET_PROTO_BYTES {
        let event_id = payload.events.len() as u64 + 1;
        let region = regions[rng.random_range(0..regions.len())].to_string();
        let route = routes[rng.random_range(0..routes.len())].to_string();
        let sku = skus[rng.random_range(0..skus.len())].to_string();
        let mut tags = Vec::with_capacity(3);
        for offset in 0..3 {
            tags.push(tag_pool[(event_id as usize + offset) % tag_pool.len()].to_string());
        }

        payload.events.push(BenchEvent {
            event_id,
            user_id: rng.random_range(1_000..10_000_000),
            shard: rng.random_range(0..256),
            timestamp_ms: 1_700_000_000_000 + event_id * 17,
            amount_cents: rng.random_range(500..250_000),
            region,
            route,
            sku,
            note: format!(
                "order={} campaign={} source=batch-{}",
                event_id,
                rng.random_range(1..1_000),
                rng.random_range(1..16)
            ),
            tags,
            success: event_id % 17 != 0,
        });
    }

    let protobuf = payload.encode_to_vec();
    let json = serde_json::to_vec(&payload).unwrap();

    SerializationCorpus {
        payload,
        protobuf,
        json,
    }
}

fn cpu_serialize_benchmark(c: &mut Criterion) {
    let corpus = serialization_corpus();
    let payload = corpus.payload;
    let protobuf = corpus.protobuf;
    let json = corpus.json;

    let mut group = c.benchmark_group("cpu_serialize");

    {
        let mut buffer = Vec::with_capacity(protobuf.len());
        group.throughput(Throughput::Bytes(protobuf.len() as u64));
        group.bench_function("protobuf/serialize", |b| {
            b.iter(|| {
                buffer.clear();
                payload.encode(&mut buffer).unwrap();
                black_box(buffer.len())
            })
        });
    }

    group.throughput(Throughput::Bytes(protobuf.len() as u64));
    group.bench_function("protobuf/deserialize", |b| {
        b.iter(|| black_box(BenchPayload::decode(protobuf.as_slice()).unwrap()))
    });

    {
        let mut buffer = Vec::with_capacity(json.len());
        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_function("json/serialize", |b| {
            b.iter(|| {
                buffer.clear();
                serde_json::to_writer(&mut buffer, &payload).unwrap();
                black_box(buffer.len())
            })
        });
    }

    group.throughput(Throughput::Bytes(json.len() as u64));
    group.bench_function("json/deserialize", |b| {
        b.iter(|| black_box(serde_json::from_slice::<BenchPayload>(&json).unwrap()))
    });

    // Into a dynamic tree, like JSON.parse in Ruby, Python or JavaScript.
    group.throughput(Throughput::Bytes(json.len() as u64));
    group.bench_function("json/deserialize to Value", |b| {
        b.iter(|| black_box(serde_json::from_slice::<serde_json::Value>(&json).unwrap()))
    });

    // A compact binary format that writes fields in order with no tags.
    let postcard = postcard::to_allocvec(&payload).unwrap();
    {
        let mut buffer = Vec::with_capacity(postcard.len());
        group.throughput(Throughput::Bytes(postcard.len() as u64));
        group.bench_function("postcard/serialize", |b| {
            b.iter(|| {
                buffer.clear();
                buffer = postcard::to_extend(&payload, std::mem::take(&mut buffer)).unwrap();
                black_box(buffer.len())
            })
        });
    }

    group.throughput(Throughput::Bytes(postcard.len() as u64));
    group.bench_function("postcard/deserialize", |b| {
        b.iter(|| black_box(postcard::from_bytes::<BenchPayload>(&postcard).unwrap()))
    });

    group.finish();
}

criterion_group!(benches, cpu_serialize_benchmark);
