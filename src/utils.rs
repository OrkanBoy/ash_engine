// This file shouldn't exist, the functions and structs should be put in their respective modules
// but they are so small that they don't deserve to :)

use core::mem::size_of;
const USIZE_BIT_COUNT: usize = 8 * size_of::<usize>();

pub fn new_bitmask_vec(bit_count: usize, all_set: bool) -> Vec<usize> {
    vec![
        if all_set { !0 } else { 0 }; 
        crate::utils::align_up(bit_count, USIZE_BIT_COUNT) / USIZE_BIT_COUNT
    ]
}

#[inline(always)]
pub const fn get_bit(bits: &[usize], index: usize) -> bool {
    bits[index / USIZE_BIT_COUNT] & (1 << (index % USIZE_BIT_COUNT)) != 0
}

#[inline(always)]
pub fn set_bit(bits: &mut [usize], index: usize, bit: bool) {
    bits[index / USIZE_BIT_COUNT] &= !(1 << (index % USIZE_BIT_COUNT));
    bits[index / USIZE_BIT_COUNT] |= (bit as usize) << (index % USIZE_BIT_COUNT);
}

#[inline(always)]
pub fn set_bit_true(bits: &mut [usize], index: usize) {
    bits[index / USIZE_BIT_COUNT] |= 1 << (index % USIZE_BIT_COUNT);
}

#[inline(always)]
pub fn set_bit_false(bits: &mut [usize], index: usize) {
    bits[index / USIZE_BIT_COUNT] &= !(1 << (index % USIZE_BIT_COUNT));
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

#[test]
fn foo_set_bit() {
    let mut bits = vec![0_usize; 4];
    set_bit(&mut bits, 4, true);

    assert!(get_bit(&bits, 4));
}