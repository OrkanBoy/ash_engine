const SIZE_OF_USIZE_AS_BITS: usize = 8 * core::mem::size_of::<usize>();

#[inline(always)]
pub const fn get_bit(bits: &[usize], index: usize) -> bool {
    bits[index / SIZE_OF_USIZE_AS_BITS] & (1 << (index % SIZE_OF_USIZE_AS_BITS)) != 0
}

#[inline(always)]
pub fn set_bit(bits: &mut [usize], index: usize, bit: bool) {
    bits[index / SIZE_OF_USIZE_AS_BITS] &= !(1 << (index % SIZE_OF_USIZE_AS_BITS));
    bits[index / SIZE_OF_USIZE_AS_BITS] |= (bit as usize) << (index % SIZE_OF_USIZE_AS_BITS);
}

#[inline(always)]
pub fn set_bit_true(bits: &mut [usize], index: usize) {
    bits[index / SIZE_OF_USIZE_AS_BITS] |= 1 << (index % SIZE_OF_USIZE_AS_BITS);
}

#[inline(always)]
pub fn set_bit_false(bits: &mut [usize], index: usize) {
    bits[index / SIZE_OF_USIZE_AS_BITS] &= !(1 << (index % SIZE_OF_USIZE_AS_BITS));
}

#[test]
fn foo_set_bit() {
    let mut bits = vec![0_usize; 4];
    set_bit(&mut bits, 4, true);

    assert!(get_bit(&bits, 4));
}