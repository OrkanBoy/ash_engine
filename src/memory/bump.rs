use crate::memory::*;
use core::ptr::null_mut;

pub struct BumpAllocator {
    pub heap_start: *mut u8,

    heap_end: *mut u8,
    next: *mut u8,
}

impl BumpAllocator {
    pub unsafe fn new(heap_start: *mut u8, heap_size: usize) -> Self {
        Self {
            heap_start,
            heap_end: (heap_start as usize + heap_size) as *mut u8,
            next: heap_start,
        }
    }
}

impl Allocator for BumpAllocator {
    unsafe fn alloc(&mut self, requested_size: usize, align: usize) -> (*mut u8, usize) {
        let next = align_up(self.next as usize, align);
        if next + requested_size >= self.heap_end as usize {
            return (null_mut(), 0);
        }
        self.next = (next + requested_size) as *mut u8;

        (next as *mut u8, requested_size)
    }

    unsafe fn dealloc(&mut self, allocated_ptr: *mut u8) {
        self.next = allocated_ptr;
    }
}