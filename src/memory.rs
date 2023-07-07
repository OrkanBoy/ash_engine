extern crate alloc;

const HEAP_SIZE: usize = 0x4000;
const HEAP_LAYOUT: core::alloc::Layout = unsafe { core::alloc::Layout::from_size_align_unchecked(HEAP_SIZE, HEAP_SIZE) };

pub mod buddy;

/// `align`: must be a power of 2
#[inline(always)]
pub const fn align_up(size: usize, align: usize) -> usize {
    (size + align - 1) & !(align - 1)
}

/// `align`: must be a power of 2
#[inline(always)]
pub const fn align_down(size: usize, align: usize) -> usize {
    size & !(align - 1)
}