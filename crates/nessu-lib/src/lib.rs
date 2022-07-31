#![deny(clippy::all)]
#![allow(clippy::new_without_default)]

mod macros;

pub mod apu;
mod bitwise;
pub mod cartridge;
pub mod cpu;
pub mod header;
pub mod input;
pub mod mapper;
pub mod nes;
pub mod op;
pub mod ppu;
