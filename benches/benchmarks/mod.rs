pub const KIB: usize = 1 << 10;
pub const MIB: usize = 1 << 20;
pub const GIB: usize = 1 << 30;

pub mod compressed_memory_read;
pub mod compression;
pub mod context_switch;
pub mod hash;
pub mod memory_latency;
pub mod memory_random;
pub mod memory_read;
pub mod serialization;
pub mod sort;
pub mod syscall;
pub mod tcp;
pub mod workload;
