pub const KIB: usize = 1 << 10;
pub const MIB: usize = 1 << 20;
pub const GIB: usize = 1 << 30;

pub mod cache_read;
pub mod cpu_atomic_mutex;
pub mod cpu_batch;
pub mod cpu_bitpack;
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

/// The first CPU after `cores[0]` on a different physical core. Clouds number SMT siblings
/// differently (n/2 apart on GCP and AWS, adjacent on some Hyper-V hosts), so ask the kernel
/// instead of assuming; outside Linux, fall back to the next CPU.
pub fn other_physical_core(cores: &[core_affinity::CoreId]) -> core_affinity::CoreId {
    let first = cores[0].id;
    let siblings = std::fs::read_to_string(format!(
        "/sys/devices/system/cpu/cpu{first}/topology/thread_siblings_list"
    ))
    .map(|list| parse_cpu_list(list.trim()))
    .unwrap_or_else(|_| vec![first]);
    *cores[1..]
        .iter()
        .find(|c| !siblings.contains(&c.id))
        .unwrap_or(&cores[1])
}

/// Parses a kernel CPU list such as "0,24" or "0-1".
fn parse_cpu_list(list: &str) -> Vec<usize> {
    list.split(',')
        .filter(|s| !s.is_empty())
        .flat_map(|range| match range.split_once('-') {
            Some((a, b)) => (a.parse().unwrap_or(0)..=b.parse().unwrap_or(0)).collect::<Vec<_>>(),
            None => range.parse().into_iter().collect(),
        })
        .collect()
}
