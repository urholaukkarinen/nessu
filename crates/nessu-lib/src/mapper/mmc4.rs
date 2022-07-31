use crate::header::Header;
use crate::mapper::{MapperTrait, Mirroring};
use crate::rand_vec;

#[derive(Clone)]
pub struct Mmc4Mapper {
    prg_ram: Vec<u8>,
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_bank0_fd: u8,
    chr_bank0_fe: u8,
    chr_bank1_fd: u8,
    chr_bank1_fe: u8,
    prg_bank: u8,
    mirroring: u8,
    latch_0: u8,
    latch_1: u8,
}

impl Mmc4Mapper {
    pub fn new(bytes: &[u8], header: &Header) -> Self {
        let prg_rom = header.prg(bytes).to_vec();
        let chr = header.chr(bytes).to_vec();

        Self {
            prg_ram: rand_vec![0x2000],
            prg_rom,
            chr,
            chr_bank0_fd: 0,
            chr_bank0_fe: 0,
            chr_bank1_fd: 1,
            chr_bank1_fe: 1,
            prg_bank: 0,
            mirroring: 0,
            latch_0: 0xFD,
            latch_1: 0xFD,
        }
    }

    fn set_prg_bank(&mut self, val: u8) {
        self.prg_bank = val & 0xF;
    }

    fn set_chr_bank0_fd(&mut self, val: u8) {
        self.chr_bank0_fd = val & 0x1F;
    }

    fn set_chr_bank0_fe(&mut self, val: u8) {
        self.chr_bank0_fe = val & 0x1F;
    }

    fn set_chr_bank1_fd(&mut self, val: u8) {
        self.chr_bank1_fd = val & 0x1F;
    }

    fn set_chr_bank1_fe(&mut self, val: u8) {
        self.chr_bank1_fe = val & 0x1F;
    }

    fn set_mirroring(&mut self, val: u8) {
        self.mirroring = val & 1;
    }
}

impl MapperTrait for Mmc4Mapper {
    fn mirroring(&self) -> Option<Mirroring> {
        Some(match self.mirroring {
            0b0 => Mirroring::Vertical,
            0b1 => Mirroring::Horizontal,
            _ => unreachable!(),
        })
    }

    fn cpu_read_u8(&mut self, addr: usize) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[addr - 0x6000],
            0x8000..=0xBFFF => self.prg_rom[addr - 0x8000 + self.prg_bank as usize * 0x4000],
            0xC000..=0xFFFF => self.prg_rom[addr - 0xC000 + self.prg_rom.len() - 0x4000],
            _ => 0,
        }
    }

    fn cpu_write_u8(&mut self, addr: usize, val: u8, _cycle: u128) {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[addr - 0x6000] = val,
            0xA000..=0xAFFF => self.set_prg_bank(val),
            0xB000..=0xBFFF => self.set_chr_bank0_fd(val),
            0xC000..=0xCFFF => self.set_chr_bank0_fe(val),
            0xD000..=0xDFFF => self.set_chr_bank1_fd(val),
            0xE000..=0xEFFF => self.set_chr_bank1_fe(val),
            0xF000..=0xFFFF => self.set_mirroring(val),
            _ => {}
        }
    }

    fn ppu_read_u8(&mut self, addr: usize) -> Option<u8> {
        match addr {
            0x0FD8..=0x0FDF => self.latch_0 = 0xFD,
            0x0FE8..=0x0FEF => self.latch_0 = 0xFE,
            0x1FD8..=0x1FDF => self.latch_1 = 0xFD,
            0x1FE8..=0x1FEF => self.latch_1 = 0xFE,
            _ => {}
        }

        #[rustfmt::skip]
        let addr = match addr {
            0x0000..=0x0FFF if self.latch_0 == 0xFD => addr + self.chr_bank0_fd as usize * 0x1000,
            0x0000..=0x0FFF if self.latch_0 == 0xFE => addr + self.chr_bank0_fe as usize * 0x1000,
            0x1000..=0x1FFF if self.latch_1 == 0xFD => addr - 0x1000 + self.chr_bank1_fd as usize * 0x1000,
            0x1000..=0x1FFF if self.latch_1 == 0xFE => addr - 0x1000 + self.chr_bank1_fe as usize * 0x1000,
            _ => return None,
        };

        Some(self.chr[addr])
    }

    fn ppu_write_u8(&mut self, addr: usize, val: u8) -> bool {
        match addr {
            0x0000..=0x1FFF => self.chr[addr] = val,
            _ => return false,
        }

        true
    }
}
