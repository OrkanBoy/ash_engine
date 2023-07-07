extern crate alloc;

use crate::memory::{Allocator, bump::BumpAllocator};

const CAPACITY_RESIZE_FACTOR: usize = 2;

use core::mem::*;
use std::ops::{IndexMut, Index};

/// `'a`: lifetime of Allocator must outlive Darray's lifetime
pub struct Darray<'a, T> {
    allocator: &'a mut dyn Allocator,

    allocated_ptr: *mut T,
    allocated_size: usize,

    capacity: usize,
    len: usize,
}

impl<'a, T> Darray<'a, T> {
    pub fn with_capacity(allocator: &'a mut dyn Allocator, capacity: usize) -> Self {
        assert!(capacity != 0);
        let (allocated_ptr, allocated_size) = unsafe {
            allocator.alloc(capacity * size_of::<T>(), align_of::<T>())
        };

        Self {
            allocator, 
            allocated_ptr: allocated_ptr as *mut T,
            allocated_size,
            capacity,
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        assert!(self.len <= self.capacity);
        if self.len == self.capacity {
            self.capacity *= CAPACITY_RESIZE_FACTOR;
            
            if self.allocated_size < self.capacity * size_of::<T>() {
                (self.allocated_ptr, self.allocated_size) = unsafe {
                    let (new_allocated_ptr, new_allocated_size) = self.allocator.alloc(
                        self.capacity * size_of::<T>(), 
                        align_of::<T>()
                    );
                    let new_allocated_ptr = new_allocated_ptr as *mut T; 

                    new_allocated_ptr.copy_from(self.allocated_ptr, self.len);
                    self.allocator.dealloc(self.allocated_ptr as *mut u8);
                    (new_allocated_ptr, new_allocated_size)
                };
            }
        }

        unsafe {
            *((self.allocated_ptr as usize + self.len * size_of::<T>()) as *mut T) = value;
        }
        self.len += 1;
    }

    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T> Index<usize> for Darray<'a, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len);
        unsafe {
            &*((self.allocated_ptr as usize + index * size_of::<T>()) as *mut T)
        }
    }
}

impl<'a, T> IndexMut<usize> for Darray<'a, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.len);
        unsafe {
            &mut *((self.allocated_ptr as usize + index * size_of::<T>()) as *mut T)
        }
    }
}

impl<'a, T> Drop for Darray<'a, T> {
    fn drop(&mut self) {
        unsafe {
            self.allocator.dealloc(self.allocated_ptr as *mut u8);
        }
    }
}

impl<'a, T: PartialEq> PartialEq for Darray<'a, T> {
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }

    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }

        for i in 0..self.len() {
            unsafe {
                if *((self.allocated_ptr as usize + i * size_of::<T>()) as *mut T) != *((other.allocated_ptr as usize + i * size_of::<T>()) as *mut T) {
                    return false;
                }
            }
        }

        true
    }
}

#[test]
fn compare_with_std() {
    let heap_size = 0x40;
    let heap_layout = unsafe { core::alloc::Layout::from_size_align_unchecked(heap_size, 1) };
    let heap_start = unsafe { alloc::alloc::alloc(heap_layout) };

    let mut allocator = unsafe {
        BumpAllocator::new(heap_start, heap_size)
    };


    {
        let std_vec = vec![6, 9, 4, 2, 0];

        let mut darray: Darray<'_, i32> = Darray::with_capacity(&mut allocator, std_vec.len());
        for &e in std_vec.iter() {
            darray.push(e);
        }
    
        for i in 0..std_vec.len() {
            assert!(darray[i] == std_vec[i]);
        }
    }

    unsafe {
        alloc::alloc::dealloc(allocator.heap_start, heap_layout);
    }
}

#[test]
fn in_place_growth() {
    let heap_size = 0x40;
    let heap_layout = unsafe { core::alloc::Layout::from_size_align_unchecked(heap_size, 1) };
    let heap_start = unsafe { alloc::alloc::alloc(heap_layout) };

    let mut allocator = unsafe {
        BumpAllocator::new(heap_start, heap_size)
    };

    {
        let std_vec = vec![6, 9, 4, 2, 0];

        let mut darray: Darray<'_, i32> = Darray::with_capacity(&mut allocator, 1);
        for &e in std_vec.iter() {
            darray.push(e);
        }
    
        for i in 0..std_vec.len() {
            assert!(darray[i] == std_vec[i]);
        }
    }

    unsafe {
        alloc::alloc::dealloc(allocator.heap_start, heap_layout);
    }
}