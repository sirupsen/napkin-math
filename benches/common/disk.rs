// Shared by the storage and storage_durable_write tools, each of which includes it with
// `#[path = "common/disk.rs"] mod disk;`. A directory without a main.rs isn't a bench target.
#![allow(dead_code)]

use std::alloc::{alloc_zeroed, dealloc, Layout};

#[cfg(target_os = "linux")]
pub const O_DIRECT: i32 = libc::O_DIRECT;
#[cfg(not(target_os = "linux"))]
pub const O_DIRECT: i32 = 0;

/// A page-aligned buffer, as O_DIRECT needs.
pub struct Buf(pub *mut u8, pub usize);
unsafe impl Send for Buf {}

impl Buf {
    pub fn new(len: usize) -> Buf {
        let ptr = unsafe { alloc_zeroed(Self::layout(len)) };
        // Not all zeroes, so no layer can skip or compress the write.
        let slice = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
        for (i, b) in slice.iter_mut().enumerate() {
            *b = (i * 31 + 7) as u8;
        }
        Buf(ptr, len)
    }

    fn layout(len: usize) -> Layout {
        Layout::from_size_align(len, 4096).unwrap()
    }

    pub fn get(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.0, self.1) }
    }

    pub fn get_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.0, self.1) }
    }
}

impl Drop for Buf {
    fn drop(&mut self) {
        unsafe { dealloc(self.0, Self::layout(self.1)) };
    }
}

/// "8k", "1m", "32g", "2t" or plain bytes.
pub fn parse_size(s: &str) -> u64 {
    let s = s.to_ascii_lowercase();
    let (num, shift) = match s.chars().last() {
        Some('k') => (&s[..s.len() - 1], 10),
        Some('m') => (&s[..s.len() - 1], 20),
        Some('g') => (&s[..s.len() - 1], 30),
        Some('t') => (&s[..s.len() - 1], 40),
        _ => (&s[..], 0),
    };
    let n: u64 = num.parse().unwrap_or_else(|_| panic!("bad size {s:?}"));
    n << shift
}
