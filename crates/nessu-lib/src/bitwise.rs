use std::ops::BitAnd;

pub trait HasBits {
    fn has_bits(&self, bits: Self) -> bool;
}

impl<T> HasBits for T
where
    T: BitAnd<Output = T> + PartialEq + Copy,
{
    #[inline]
    fn has_bits(&self, bits: Self) -> bool {
        *self & bits == bits
    }
}

pub trait HiLoBytes {
    fn high_u8(&self) -> u8;
    fn low_u8(&self) -> u8;
}

impl HiLoBytes for u16 {
    #[inline]
    fn high_u8(&self) -> u8 {
        (self >> 8) as u8
    }
    #[inline]
    fn low_u8(&self) -> u8 {
        (self & 0xFF) as u8
    }
}

pub trait IsEven {
    fn is_even(&self) -> bool;
    fn is_odd(&self) -> bool;
}

macro_rules! impl_is_even {
    (for $($t:ty),+) => {
        $(impl IsEven for $t {
            #[inline]
            fn is_even(&self) -> bool {
                self & 1 == 0
            }

            #[inline]
            fn is_odd(&self) -> bool {
                !self.is_even()
            }
        })*
    }
}

impl_is_even!(for usize, u16);
