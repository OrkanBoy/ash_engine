use crate::memory::buddy::Allocator;

use core::mem::*;
use std::ops::{IndexMut, Index};

pub struct Darray<'a, T> {
    allocator: &'a mut Allocator,

    data: *mut T,
    capacity: usize,
    len: usize,
}

impl<'a, T> Darray<'a, T> {
    pub fn with_capacity(allocator: &'a mut Allocator, capacity: usize) -> Self {
        let data = unsafe {
            allocator.alloc(capacity * size_of::<T>(), align_of::<T>())
        } as *mut T;

        Self {
            allocator,
            data,
            capacity,
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        // TODO: grow in place if space available
        unsafe {
            *((self.data as usize + self.len * size_of::<T>()) as *mut T) = value;
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
            &*((self.data as usize + index * size_of::<T>()) as *mut T)
        }
    }
}

impl<'a, T> IndexMut<usize> for Darray<'a, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.len);
        unsafe {
            &mut *((self.data as usize + index * size_of::<T>()) as *mut T)
        }
    }
}

impl<'a, T> Drop for Darray<'a, T> {
    fn drop(&mut self) {
        unsafe {
            self.allocator.dealloc(self.data as *mut u8);
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
                if *((self.data as usize + i * size_of::<T>()) as *mut T) != *((other.data as usize + i * size_of::<T>()) as *mut T) {
                    return false;
                }
            }
        }

        true
    }
}

#[test]
fn darray_test() {
    let mut allocator = unsafe {
        crate::memory::buddy::Allocator::new()
    };

    let std_vec = vec![6, 9, 4, 2, 0];

    let mut darray: Darray<'_, i32> = Darray::with_capacity(&mut allocator, std_vec.len());
    for &e in std_vec.iter() {
        darray.push(e);
    }

    for i in 0..std_vec.len() {
        assert!(darray[i] == std_vec[i]);
    }
}