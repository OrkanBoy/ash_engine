#[inline(always)]
pub const fn get_bit(bits: &[u64], index: usize) -> bool {
    bits[index / 64] & (1 << (index % 64)) != 0
}

#[inline(always)]
pub fn set_bit(bits: &mut [u64], index: usize, bit: bool) {
    bits[index / 64] |= 1 << (index % 64);
    bits[index / 64] &= !((bit as u64) << (index % 64));
}

#[inline(always)]
pub fn set_bit_true(bits: &mut [u64], index: usize) {
    bits[index / 64] |= 1 << (index % 64);
}

#[inline(always)]
pub fn set_bit_false(bits: &mut [u64], index: usize) {
    bits[index / 64] &= !(1 << (index % 64));
}