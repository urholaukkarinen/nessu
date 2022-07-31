use crate::bitwise::HasBits;
use crate::header::Header;
use crate::mapper::{MapperTrait, Mirroring};
use crate::rand_vec;

#[derive(Clone)]
pub struct Mmc1Mapper {
    prg_ram: Vec<u8>,
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_bank0: u8,
    chr_bank1: u8,

    prg_bank: u8,
    prg_bank_mode: u8,
    chr_bank_mode: u8,

    mirroring: u8,

    shift_register: u8,

    prev_write_cycle: u128,
}

impl Mmc1Mapper {
    pub fn new(bytes: &[u8], header: &Header) -> Self {
        let prg_rom = header.prg(bytes).to_vec();
        let mut chr = vec![0; 0x20000];

        if header.chr_size > 0 {
            let chr_in = header.chr(bytes);
            chr[..chr_in.len()].copy_from_slice(chr_in);
        }

        Self {
            prg_ram: rand_vec![2 << 13],
            prg_rom,
            chr,
            chr_bank0: 0,
            chr_bank1: 1,
            prg_bank: 0,
            prg_bank_mode: 3,
            chr_bank_mode: 0,
            mirroring: 0,
            shift_register: 0b10000,
            prev_write_cycle: u128::MAX - 1,
        }
    }

    fn control_register(&self) -> u8 {
        self.mirroring | (self.prg_bank_mode << 2) | (self.chr_bank_mode << 4)
    }

    fn set_control_register(&mut self, val: u8) {
        let prg_bank_mode = (val >> 2) & 0b11;
        if prg_bank_mode != self.prg_bank_mode {
            self.prg_bank_mode = prg_bank_mode;

            log::debug!("prg_bank_mode changed: 0b{:02b}", prg_bank_mode);
        }

        let chr_bank_mode = (val >> 4) & 0b1;
        if chr_bank_mode != self.chr_bank_mode {
            self.chr_bank_mode = chr_bank_mode;

            log::debug!("chr_bank_mode changed: 0b{:01b}", chr_bank_mode);
        }

        let mirroring = val & 0b11;
        if mirroring != self.mirroring {
            self.mirroring = mirroring;

            log::debug!("mirroning changed: 0b{:02b}", mirroring);
        }
    }

    fn set_chr_bank0(&mut self, val: u8) {
        let chr_bank0 = val & 0b11111;
        if chr_bank0 != self.chr_bank0 {
            self.chr_bank0 = chr_bank0;
            log::debug!("chr_bank0 changed: 0b{:02b}", chr_bank0);
        }
    }

    fn set_chr_bank1(&mut self, val: u8) {
        let chr_bank1 = val & 0b11111;
        if chr_bank1 != self.chr_bank1 {
            self.chr_bank1 = chr_bank1;
            log::debug!("chr_bank1 changed: 0b{:02b}", chr_bank1);
        }
    }

    fn set_prg_bank(&mut self, val: u8) {
        self.prg_bank = val & 0b1111;
    }

    fn write_load_register(&mut self, addr: usize, val: u8) {
        if val >> 7 == 1 {
            self.shift_register = 0b10000;
            self.set_control_register(self.control_register() | 0xC0);
        } else {
            let full = self.shift_register.has_bits(0b1);

            self.shift_register >>= 1;
            self.shift_register |= (val & 1) << 4;

            if full {
                let val = self.shift_register & 0x1F;
                self.shift_register = 0b10000;

                match 0x8000 | (addr & 0x6000) {
                    0x8000 => self.set_control_register(val),
                    0xA000 => self.set_chr_bank0(val),
                    0xC000 => self.set_chr_bank1(val),
                    0xE000 => self.set_prg_bank(val),
                    _ => unreachable!(),
                }
            }
        }
    }

    fn effective_ppu_addr(&self, addr: usize) -> usize {
        match addr {
            0x0000..=0x1FFF if self.chr_bank_mode == 0 => {
                addr + ((self.chr_bank0 & !1) as usize * 0x1000)
            }
            0x0000..=0x0FFF if self.chr_bank_mode == 1 => addr + (self.chr_bank0 as usize * 0x1000),
            0x1000..=0x1FFF if self.chr_bank_mode == 1 => {
                addr - 0x1000 + (self.chr_bank1 as usize * 0x1000)
            }
            _ => addr,
        }
    }
}

impl MapperTrait for Mmc1Mapper {
    fn mirroring(&self) -> Option<Mirroring> {
        Some(match self.mirroring {
            0b00 => Mirroring::OneScreenLowerBank,
            0b01 => Mirroring::OneScreenUpperBank,
            0b10 => Mirroring::Vertical,
            0b11 => Mirroring::Horizontal,
            _ => unreachable!(),
        })
    }

    fn cpu_read_u8(&mut self, addr: usize) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[addr - 0x6000],
            0x8000..=0xFFFF if self.prg_bank_mode == 0 || self.prg_bank_mode == 1 => {
                self.prg_rom[addr - 0x8000 + ((self.prg_bank & !1) as usize * 0x4000)]
            }
            0x8000..=0xBFFF => {
                if self.prg_bank_mode == 2 {
                    self.prg_rom[addr - 0x8000]
                } else if self.prg_bank_mode == 3 {
                    self.prg_rom[addr - 0x8000 + (self.prg_bank as usize * 0x4000)]
                } else {
                    panic!(
                        "Not implemented: tried to read from 0x{:04X} with bank mode {}",
                        addr, self.prg_bank_mode
                    );
                }
            }
            0xC000..=0xFFFF => {
                if self.prg_bank_mode == 2 {
                    self.prg_rom[addr - 0xC000 + (self.prg_bank as usize * 0x4000)]
                } else if self.prg_bank_mode == 3 {
                    self.prg_rom[addr - 0xC000 + self.prg_rom.len() - 0x4000]
                } else {
                    panic!(
                        "Not implemented: tried to read from 0x{:04X} with bank mode {}",
                        addr, self.prg_bank_mode
                    );
                }
            }
            _ => 0,
        }
    }

    fn cpu_write_u8(&mut self, addr: usize, val: u8, cycle: u128) {
        let consecutive_write = cycle == self.prev_write_cycle + 1;
        self.prev_write_cycle = cycle;

        if consecutive_write {
            return;
        }

        match addr {
            0x6000..=0x7FFF => self.prg_ram[addr - 0x6000] = val,
            0x8000..=0xFFFF => self.write_load_register(addr, val),
            _ => {}
        }
    }

    fn ppu_read_u8(&mut self, addr: usize) -> Option<u8> {
        match addr {
            0x0000..=0x1FFF => {
                let addr = self.effective_ppu_addr(addr);
                Some(self.chr[addr])
            }
            _ => None,
        }
    }

    fn ppu_write_u8(&mut self, addr: usize, val: u8) -> bool {
        match addr {
            0x0000..=0x1FFF => {
                let addr = self.effective_ppu_addr(addr);
                self.chr[addr] = val;
                true
            }
            _ => false,
        }
    }
}
