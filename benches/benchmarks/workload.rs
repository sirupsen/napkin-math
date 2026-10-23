use prost::Message;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

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

pub struct CompressionCorpus {
    pub bytes: Vec<u8>,
    pub compressed: Vec<u8>,
    pub level: i32,
}

pub fn serialization_corpus() -> SerializationCorpus {
    const TARGET_PROTO_BYTES: usize = n_mib_bytes!(1) as usize;
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
        let region = regions[rng.gen_range(0, regions.len())].to_string();
        let route = routes[rng.gen_range(0, routes.len())].to_string();
        let sku = skus[rng.gen_range(0, skus.len())].to_string();
        let mut tags = Vec::with_capacity(3);
        for offset in 0..3 {
            tags.push(tag_pool[(event_id as usize + offset) % tag_pool.len()].to_string());
        }

        payload.events.push(BenchEvent {
            event_id,
            user_id: rng.gen_range(1_000, 10_000_000),
            shard: rng.gen_range(0, 256),
            timestamp_ms: 1_700_000_000_000 + event_id * 17,
            amount_cents: rng.gen_range(500, 250_000),
            region,
            route,
            sku,
            note: format!(
                "order={} campaign={} source=batch-{}",
                event_id,
                rng.gen_range(1, 1_000),
                rng.gen_range(1, 16)
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

pub fn compression_corpus() -> CompressionCorpus {
    const TARGET_UNCOMPRESSED_BYTES: usize = n_mib_bytes!(4) as usize;
    const LEVEL: i32 = 3;

    let serialization = serialization_corpus();
    let mut bytes = Vec::with_capacity(TARGET_UNCOMPRESSED_BYTES);
    while bytes.len() < TARGET_UNCOMPRESSED_BYTES {
        bytes.extend_from_slice(&serialization.json);
    }
    bytes.truncate(TARGET_UNCOMPRESSED_BYTES);

    let compressed = zstd::bulk::compress(&bytes, LEVEL).unwrap();

    CompressionCorpus {
        bytes,
        compressed,
        level: LEVEL,
    }
}
