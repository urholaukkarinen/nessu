use crate::header::Header;
use crate::mapper::{MapperTrait, Mirroring};

#[derive(Clone)]
pub struct NromMapper {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    prg_mirrored: bool,
}

impl NromMapper {
    pub fn new(bytes: &[u8], header: &Header) -> Self {
        let prg_rom = if header.prg_size > 0 {
            bytes[header.prg_start as usize..header.prg_end as usize].to_vec()
        } else {
            vec![0; 0x4000]
        };

        let mut chr = vec![0; 0x2000];
        header.copy_chr(bytes, &mut chr);

        let prg_mirrored = prg_rom.len() <= 0x4000;

        Self {
            prg_rom,
            chr,
            prg_mirrored,
        }
    }

    fn effective_cpu_addr(&self, addr: usize) -> usize {
        match addr {
            0xC000..=0xFFFF if self.prg_mirrored => addr & 0xBFFF,
            _ => addr,
        }
    }
}

impl MapperTrait for NromMapper {
    fn mirroring(&self) -> Option<Mirroring> {
        None
    }

    fn cpu_read_u8(&mut self, addr: usize) -> u8 {
        let addr = self.effective_cpu_addr(addr) as usize;
        match addr {
            0x8000..=0xFFFF if addr - 0x8000 < self.prg_rom.len() => self.prg_rom[addr - 0x8000],
            _ => 0,
        }
    }

    fn cpu_write_u8(&mut self, addr: usize, val: u8, _cycle: u128) {
        let addr = self.effective_cpu_addr(addr) as usize;
        match addr {
            0x8000..=0xFFFF if addr - 0x8000 < self.prg_rom.len() => {
                self.prg_rom[addr - 0x8000] = val
            }
            _ => {}
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
