pub const KIB: usize = 1 << 10;
pub const MIB: usize = 1 << 20;
pub const GIB: usize = 1 << 30;

pub mod cache_read;
pub mod cpu_atomic_mutex;
pub mod cpu_context_switch;
pub mod cpu_encrypt;
pub mod cpu_handshake;
pub mod cpu_hash;
pub mod cpu_serialize;
pub mod cpu_sort;
pub mod cpu_spawn;
pub mod cpu_system_call;
pub mod memory_random;
pub mod memory_sequential_read;
pub mod memory_sequential_write;
