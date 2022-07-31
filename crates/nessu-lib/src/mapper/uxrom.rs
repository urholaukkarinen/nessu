use crate::header::Header;
use crate::mapper::{MapperTrait, Mirroring};

#[derive(Clone)]
pub struct UxRomMapper {
    prg_rom: Vec<u8>,
    prg_bank0: u8,
    chr: Vec<u8>,
}

impl UxRomMapper {
    pub fn new(bytes: &[u8], header: &Header) -> Self {
        let prg_rom = bytes[header.prg_start as usize..header.prg_end as usize].to_vec();

        let mut chr = vec![0; 0x2000];
        header.copy_chr(bytes, &mut chr);

        Self {
            prg_rom,
            prg_bank0: 0,
            chr,
        }
    }
}

impl MapperTrait for UxRomMapper {
    fn mirroring(&self) -> Option<Mirroring> {
        None
    }

    fn cpu_read_u8(&mut self, addr: usize) -> u8 {
        match addr {
            0x8000..=0xBFFF => self.prg_rom[addr - 0x8000 + ((self.prg_bank0 as usize) << 14)],
            0xC000..=0xFFFF => self.prg_rom[addr + 0x10000],
            _ => 0,
        }
    }

    fn cpu_write_u8(&mut self, addr: usize, val: u8, _cycle: u128) {
        if let 0x8000..=0xFFFF = addr {
            self.prg_bank0 = (val & 0b111) as u8;
        }
    }

    fn ppu_read_u8(&mut self, addr: usize) -> Option<u8> {
        match addr {
            0x0000..=0x1FFF => Some(self.chr[addr]),
            _ => None,
        }
    }

    fn ppu_write_u8(&mut self, addr: usize, val: u8) -> bool {
        match addr {
            0x0000..=0x1FFF => self.chr[addr] = val,
            _ => return false,
        }

        true
    }
}
