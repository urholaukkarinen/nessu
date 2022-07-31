use crate::bitwise::HasBits;
use crate::mapper::{MapperKind, Mirroring};
use std::io::{Error, ErrorKind};

#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Header {
    pub prg_size: u8,
    pub chr_size: u8,
    pub flags6: u8,
    pub flags7: u8,
    pub mirroring: Mirroring,
    pub mapper: MapperKind,
    pub prg_start: usize,
    pub prg_end: usize,
    pub chr_start: usize,
    pub chr_end: usize,
    pub persistence: bool,
}

impl Header {
    pub fn read_from_slice(slice: &[u8]) -> std::io::Result<Self> {
        if slice.len() < 16 {
            return Err(Error::from(ErrorKind::InvalidData));
        }

        if slice[0..4] != [0x4E, 0x45, 0x53, 0x1A] {
            // Header should start with "NES"
            return Err(Error::from(ErrorKind::InvalidData));
        }

        let prg_size = slice[4];
        let chr_size = slice[5];
        let flags6 = slice[6];
        let flags7 = slice[7];

        if (flags7 >> 2) & 0b11 == 2 {
            eprintln!("NES 2.0 not supported yet");
            return Err(Error::from(ErrorKind::Unsupported));
        }

        let mirroring = if flags6 & 1 == 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        };

        if (flags6 >> 3) & 1 == 1 {
            eprintln!("TODO: Ignore mirroring control or above mirroring bit; instead provide four-screen FVRAM");
            return Err(Error::from(ErrorKind::Unsupported));
        }

        let persistence = flags6.has_bits(0b10);

        let mapper = MapperKind::from((flags6 >> 4) | (flags7 & 0xF0));

        let prg_start = if ((flags6 >> 0x2) & 0x1) == 0x1 {
            0x210
        } else {
            0x10
        };

        let prg_end = prg_start + prg_size as usize * 0x4000;

        let chr_start = prg_end;
        let chr_end = chr_start + chr_size as usize * 0x2000;

        Ok(Self {
            prg_size,
            chr_size,
            flags6,
            flags7,
            mirroring,
            mapper,
            prg_start,
            prg_end,
            chr_start,
            chr_end,
            persistence,
        })
    }

    pub fn copy_chr(&self, src: &[u8], dst: &mut [u8]) {
        if self.chr_size > 0 {
            dst[0..=(self.chr_end - self.chr_start - 1)]
                .copy_from_slice(&src[self.chr_start as usize..self.chr_end as usize]);
        }
    }

    pub fn copy_prg(&self, src: &[u8], dst: &mut [u8]) {
        if self.prg_size > 0 {
            dst[0..=(self.prg_end - self.prg_start - 1)]
                .copy_from_slice(&src[self.prg_start as usize..self.prg_end as usize]);
        }
    }

    pub fn chr<'a>(&self, src: &'a [u8]) -> &'a [u8] {
        &src[self.chr_start as usize..self.chr_end as usize]
    }

    pub fn prg<'a>(&self, src: &'a [u8]) -> &'a [u8] {
        &src[self.prg_start as usize..self.prg_end as usize]
    }
}
