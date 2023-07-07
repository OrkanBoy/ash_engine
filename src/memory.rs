extern crate alloc;

use core::mem::{size_of, align_of};

pub mod buddy;
pub mod bump;
pub mod free_list;

pub trait Allocator {
    /// `align`: must be a power of 2.
    /// `size`:  must be up aligned with `align`.
    /// returns allocated ptr and allocated size
    unsafe fn alloc(&mut self, requested_size: usize, requested_align: usize) -> (*mut u8, usize);
    /// `ptr`: must be returned from a previous alloc call
    unsafe fn dealloc(&mut self, ptr: *mut u8);
}

pub unsafe fn set_array_element<T>(array: *mut T, index: usize, val: T) {
    *((array as usize + index * size_of::<T>()) as *mut T) = val;
}

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

/// `align`: must be a power of 2
#[inline(always)]
pub fn align_ptr_up(ptr: *mut u8, align: usize) -> *mut u8 {
    ((ptr as usize + align - 1) & !(align - 1)) as *mut u8
}

/// `align`: must be a power of 2
#[inline(always)]
pub fn align_ptr_down(ptr: *mut u8, align: usize) -> *mut u8 {
    (ptr as usize & !(align - 1)) as *mut u8
}