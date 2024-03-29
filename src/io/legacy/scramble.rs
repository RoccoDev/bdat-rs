use std::num::Wrapping;

#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Clone, Copy)]
pub enum ScrambleType {
    None,
    Scrambled(u16),
}

/// Unscrambles a section of legacy BDAT data.
#[inline]
pub fn unscramble(data: &mut [u8], key: u16) {
    unscramble_chunks(data, key)
}

/// Scrambles a section of legacy BDAT data.
#[inline]
pub fn scramble(data: &mut [u8], key: u16) {
    scramble_chunks(data, key)
}

/// Calculates the checksum for an unscrambled BDAT table.  
/// The checksum can then be used as the scramble key.
pub fn calc_checksum(full_table: &[u8]) -> u16 {
    if full_table.len() <= 0x20 {
        return 0;
    }
    full_table[0x20..]
        .iter()
        .enumerate()
        .map(|(idx, b)| Wrapping(u16::from(b.wrapping_shl((idx & 3) as u32))))
        .sum::<Wrapping<_>>()
        .0
}

// Various (un)scramble implementations - all correct (unit-tested below) and benchmarked
// (see benches/scramble.rs)

#[cfg(any(test, feature = "bench"))]
#[inline]
pub fn unscramble_naive(data: &mut [u8], key: u16) {
    let mut t1 = ((!key >> 8) & 0xff) as u8;
    let mut t2 = (!key & 0xff) as u8;
    let mut i = 0;
    while i < data.len() - 1 {
        let a = data[i];
        let b = data[i + 1];
        data[i] ^= t1;
        data[i + 1] ^= t2;
        t1 = t1.wrapping_add(a);
        t2 = t2.wrapping_add(b);
        i += 2;
    }
    if i < data.len() {
        // odd size
        data[i] ^= t1;
    }
}

//#[cfg(any(test, feature = "bench"))]
#[inline]
pub fn unscramble_chunks(data: &mut [u8], key: u16) {
    let mut t1 = ((key >> 8) ^ 0xff) as u8;
    let mut t2 = (key ^ 0xff) as u8;
    let mut chunks = data.chunks_exact_mut(2);
    for x in &mut chunks {
        let [a, b, ..] = x else { unreachable!() };
        let old_a = *a;
        let old_b = *b;
        *a ^= t1;
        *b ^= t2;
        t1 = t1.wrapping_add(old_a);
        t2 = t2.wrapping_add(old_b);
    }
    if let Some(x) = chunks.into_remainder().get_mut(0) {
        // odd size
        *x ^= t1;
    }
}

#[inline]
pub fn scramble_chunks(data: &mut [u8], key: u16) {
    let mut t1 = ((key >> 8) ^ 0xff) as u8;
    let mut t2 = (key ^ 0xff) as u8;
    let mut chunks = data.chunks_exact_mut(2);
    for x in &mut chunks {
        let [a, b, ..] = x else { unreachable!() };
        *a ^= t1;
        *b ^= t2;
        t1 = t1.wrapping_add(*a);
        t2 = t2.wrapping_add(*b);
    }
    if let Some(x) = chunks.into_remainder().get_mut(0) {
        // odd size
        *x ^= t1;
    }
}

#[cfg(any(test, feature = "bench"))]
#[inline(never)] // worse performance
pub fn unscramble_single(data: &mut [u8], key: u16) {
    let mut t1 = ((key >> 8) ^ 0xff) as u8;
    let mut t2 = (key ^ 0xff) as u8;
    let mut key = &mut t1;
    let mut b = false;
    for x in data {
        let old = *x;
        *x ^= *key;
        *key = key.wrapping_add(old);
        key = if b { &mut t1 } else { &mut t2 };
        b = !b;
    }
}

#[cfg(any(test, feature = "bench"))]
pub mod tests {
    pub const INPUT: [u8; 14] = [
        0xfb, 0x7e, 0xe4, 0xf1, 0xe4, 0xeb, 0x4b, 0xba, 0xf4, 0x75, 0xe7, 0xd4, 0xec, 0x8d,
    ];

    pub const INPUT_NO_NUL: [u8; 13] = [
        0xfb, 0x7e, 0xe4, 0xf1, 0xe4, 0xeb, 0x4b, 0xba, 0xf4, 0x75, 0xe7, 0xd4, 0xec,
    ];

    // "MNU_qt2001_ms\0"
    const EXPECTED: [u8; 14] = [
        0x4d, 0x4e, 0x55, 0x5f, 0x71, 0x74, 0x32, 0x30, 0x30, 0x31, 0x5f, 0x6d, 0x73, 0x00,
    ];

    const EXPECTED_NO_NUL: [u8; 13] = [
        0x4d, 0x4e, 0x55, 0x5f, 0x71, 0x74, 0x32, 0x30, 0x30, 0x31, 0x5f, 0x6d, 0x73,
    ];

    pub const KEY: u16 = 0x49cf;

    #[test]
    fn naive() {
        assert(super::unscramble_naive);
    }

    #[test]
    fn chunks() {
        assert(super::unscramble_chunks);
    }

    #[test]
    fn single() {
        assert(super::unscramble_single);
    }

    #[test]
    fn scramble_naive() {
        assert_reverse(super::scramble_chunks);
    }

    #[test]
    fn checksum() {
        let mut table = vec![0u8; 0x20];
        table.extend_from_slice(&EXPECTED);
        let sum = super::calc_checksum(&table);
        assert_eq!(1727, sum);
    }

    fn assert(f: fn(&mut [u8], u16)) {
        let mut data = INPUT;
        f(&mut data, KEY);
        assert_eq!(data, EXPECTED);
        let mut data = INPUT_NO_NUL;
        f(&mut data, KEY);
        assert_eq!(data, EXPECTED_NO_NUL);
    }

    fn assert_reverse(f: fn(&mut [u8], u16)) {
        let mut data = EXPECTED;
        f(&mut data, KEY);
        assert_eq!(data, INPUT);
    }
}
