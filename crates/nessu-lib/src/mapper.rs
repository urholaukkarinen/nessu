mod mmc1;
mod mmc3;
mod mmc4;
mod nrom;
mod uxrom;

use enum_dispatch::enum_dispatch;

use crate::header::Header;
use crate::mapper::mmc1::Mmc1Mapper;
use crate::mapper::mmc3::Mmc3Mapper;
use crate::mapper::mmc4::Mmc4Mapper;
use crate::mapper::nrom::NromMapper;
use crate::mapper::uxrom::UxRomMapper;
use std::io::ErrorKind;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum Mirroring {
    OneScreenLowerBank,
    OneScreenUpperBank,
    Horizontal,
    Vertical,
}

impl Default for Mirroring {
    fn default() -> Self {
        Mirroring::OneScreenLowerBank
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum MapperKind {
    NROM,
    MMC1,
    UXROM,
    MMC3,
    MMC4,
    Unknown(u8),
}

impl Default for MapperKind {
    fn default() -> Self {
        MapperKind::NROM
    }
}

impl From<u8> for MapperKind {
    fn from(val: u8) -> Self {
        match val {
            0 => MapperKind::NROM,
            1 => MapperKind::MMC1,
            2 => MapperKind::UXROM,
            4 => MapperKind::MMC3,
            10 => MapperKind::MMC4,
            val => MapperKind::Unknown(val),
        }
    }
}

#[enum_dispatch]
#[derive(Clone)]
pub enum Mapper {
    NromMapper,
    Mmc1Mapper,
    UxRomMapper,
    Mmc3Mapper,
    Mmc4Mapper,
}

pub fn build_mapper(data: &[u8], header: &Header) -> std::io::Result<Mapper> {
    match header.mapper {
        MapperKind::NROM => Ok(NromMapper::new(data, header).into()),
        MapperKind::MMC1 => Ok(Mmc1Mapper::new(data, header).into()),
        MapperKind::UXROM => Ok(UxRomMapper::new(data, header).into()),
        MapperKind::MMC3 => Ok(Mmc3Mapper::new(data, header).into()),
        MapperKind::MMC4 => Ok(Mmc4Mapper::new(data, header).into()),
        MapperKind::Unknown(val) => {
            eprintln!("Unsupported mapper: {}", val);
            Err(std::io::Error::from(ErrorKind::Unsupported))
        }
    }
}

#[enum_dispatch(Mapper)]
pub trait MapperTrait {
    fn mirroring(&self) -> Option<Mirroring>;
    fn cpu_read_u8(&mut self, addr: usize) -> u8;
    fn cpu_write_u8(&mut self, addr: usize, val: u8, _cycle: u128);
    fn ppu_read_u8(&mut self, addr: usize) -> Option<u8>;
    fn ppu_write_u8(&mut self, addr: usize, val: u8) -> bool;

    fn irq_triggered(&mut self) -> bool {
        false
    }

    fn clock_irq(&mut self) {}
}
