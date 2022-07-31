use crate::bitwise::{HasBits, IsEven};
use crate::header::Header;
use crate::mapper::{MapperTrait, Mirroring};
use crate::rand_vec;

#[derive(Clone)]
pub struct Mmc3Mapper {
    prg_ram: Vec<u8>,
    prg_rom: Vec<u8>,
    chr: Vec<u8>,

    r: [u8; 8],

    prg_bank_8000: usize,
    prg_bank_a000: usize,
    prg_bank_c000: usize,
    prg_bank_e000: usize,
    chr_r0: usize,
    chr_r1: usize,
    chr_r2: usize,
    chr_r3: usize,
    chr_r4: usize,
    chr_r5: usize,
    mirroring: Mirroring,
    prg_ram_enabled: bool,
    prg_ram_read_only: bool,
    next_bank_update: u8,
    prg_rom_mode: u8,
    chr_a12_inversion: u8,
    irq_reload: u8,
    irq_counter: u8,
    irq_enabled: bool,
    irq_triggered: bool,
}

impl Mmc3Mapper {
    pub fn new(bytes: &[u8], header: &Header) -> Self {
        let prg_rom = header.prg(bytes).to_vec();
        let mut chr = vec![0; 0x40000];
        header.copy_chr(bytes, &mut chr);

        let prg_bank_8000 = 0x0000;
        let prg_bank_a000 = 0x2000;
        let prg_bank_c000 = prg_rom.len() - 0x4000;
        let prg_bank_e000 = prg_rom.len() - 0x2000;

        Self {
            r: [0; 8],
            prg_ram: rand_vec![0x2000],
            prg_rom,
            chr,
            prg_bank_8000,
            prg_bank_a000,
            prg_bank_c000,
            prg_bank_e000,
            chr_r0: 0,
            chr_r1: 0,
            chr_r2: 0,
            chr_r3: 0,
            chr_r4: 0,
            chr_r5: 0,
            mirroring: Mirroring::Horizontal,
            prg_ram_enabled: false,
            prg_ram_read_only: false,
            next_bank_update: 0,
            prg_rom_mode: 0,
            chr_a12_inversion: 0,
            irq_reload: 0,
            irq_counter: 0,
            irq_enabled: false,
            irq_triggered: false,
        }
    }

    fn bank_select(&mut self, val: u8) {
        self.next_bank_update = val & 0b111;
        self.prg_rom_mode = (val >> 6) & 1;
        self.chr_a12_inversion = (val >> 7) & 1;
    }

    fn set_bank_data(&mut self, val: u8) {
        self.r[self.next_bank_update as usize] = val;

        self.chr_r0 = (self.r[0] & !1) as usize * 0x0400;
        self.chr_r1 = (self.r[1] & !1) as usize * 0x0400;
        self.chr_r2 = self.r[2] as usize * 0x0400;
        self.chr_r3 = self.r[3] as usize * 0x0400;
        self.chr_r4 = self.r[4] as usize * 0x0400;
        self.chr_r5 = self.r[5] as usize * 0x0400;

        if self.prg_rom_mode == 0 {
            self.prg_bank_8000 = (self.r[6] & 0x3F) as usize * 0x2000;
            self.prg_bank_c000 = self.prg_rom.len() - 0x4000;
        } else {
            self.prg_bank_8000 = self.prg_rom.len() - 0x4000;
            self.prg_bank_c000 = (self.r[6] & 0x3F) as usize * 0x2000;
        }

        self.prg_bank_a000 = (self.r[7] & 0x3F) as usize * 0x2000;
    }

    fn set_mirroring(&mut self, val: u8) {
        self.mirroring = match val & 0b1 {
            0b0 => Mirroring::Vertical,
            0b1 => Mirroring::Horizontal,
            _ => unreachable!(),
        };
    }

    fn prg_ram_protect(&mut self, val: u8) {
        self.prg_ram_enabled = val.has_bits(0x80);
        self.prg_ram_read_only = val.has_bits(0x40);
    }

    fn set_irq_latch(&mut self, val: u8) {
        self.irq_reload = val;
    }

    fn reset_irq_counter(&mut self) {
        self.irq_counter = 0;
    }

    fn enable_irq(&mut self) {
        self.irq_enabled = true;
    }

    fn disable_irq(&mut self) {
        self.irq_enabled = false;
        self.irq_triggered = false;
    }

    fn effective_ppu_addr(&mut self, addr: usize) -> Option<usize> {
        match addr {
            0x0000..=0x07FF if self.chr_a12_inversion == 0 => Some(addr + self.chr_r0),
            0x0800..=0x0FFF if self.chr_a12_inversion == 0 => Some((addr - 0x0800) + self.chr_r1),
            0x1000..=0x13FF if self.chr_a12_inversion == 0 => Some((addr - 0x1000) + self.chr_r2),
            0x1400..=0x17FF if self.chr_a12_inversion == 0 => Some((addr - 0x1400) + self.chr_r3),
            0x1800..=0x1BFF if self.chr_a12_inversion == 0 => Some((addr - 0x1800) + self.chr_r4),
            0x1C00..=0x1FFF if self.chr_a12_inversion == 0 => Some((addr - 0x1C00) + self.chr_r5),

            0x0000..=0x03FF if self.chr_a12_inversion == 1 => Some(addr + self.chr_r2),
            0x0400..=0x07FF if self.chr_a12_inversion == 1 => Some((addr - 0x0400) + self.chr_r3),
            0x1000..=0x17FF if self.chr_a12_inversion == 1 => Some((addr - 0x1000) + self.chr_r0),
            0x1800..=0x1FFF if self.chr_a12_inversion == 1 => Some((addr - 0x1800) + self.chr_r1),
            0x0800..=0x0BFF if self.chr_a12_inversion == 1 => Some((addr - 0x0800) + self.chr_r4),
            0x0C00..=0x0FFF if self.chr_a12_inversion == 1 => Some((addr - 0x0C00) + self.chr_r5),
            _ => None,
        }
    }
}

impl MapperTrait for Mmc3Mapper {
    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.mirroring)
    }

    #[rustfmt::skip]
    fn cpu_read_u8(&mut self, addr: usize) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                // TODO return open bus if disabled
                self.prg_ram[addr & 0x1FFF]
            }
            0x8000..=0x9FFF => self.prg_rom[(addr & 0x1FFF) + self.prg_bank_8000],
            0xA000..=0xBFFF => self.prg_rom[(addr & 0x1FFF) + self.prg_bank_a000],
            0xC000..=0xDFFF => self.prg_rom[(addr & 0x1FFF) + self.prg_bank_c000],
            0xE000..=0xFFFF => self.prg_rom[(addr & 0x1FFF) + self.prg_bank_e000],
            _ => 0,
        }
    }

    fn cpu_write_u8(&mut self, addr: usize, val: u8, _cycle: u128) {
        match addr {
            0x6000..=0x7FFF if !self.prg_ram_read_only => self.prg_ram[addr - 0x6000] = val,
            0x8000..=0x9FFE if addr.is_even() => self.bank_select(val),
            0x8001..=0x9FFF if addr.is_odd() => self.set_bank_data(val),
            0xA000..=0xBFFE if addr.is_even() => self.set_mirroring(val),
            0xA001..=0xBFFF if addr.is_odd() => self.prg_ram_protect(val),
            0xC000..=0xDFFE if addr.is_even() => self.set_irq_latch(val),
            0xC001..=0xDFFF if addr.is_odd() => self.reset_irq_counter(),
            0xE000..=0xFFFE if addr.is_even() => self.disable_irq(),
            0xE001..=0xFFFF if addr.is_odd() => self.enable_irq(),
            _ => {}
        }
    }

    fn ppu_read_u8(&mut self, addr: usize) -> Option<u8> {
        self.effective_ppu_addr(addr).map(|addr| self.chr[addr])
    }

    fn ppu_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn irq_triggered(&mut self) -> bool {
        std::mem::take(&mut self.irq_triggered)
    }

    fn clock_irq(&mut self) {
        if self.irq_counter == 0 {
            self.irq_counter = self.irq_reload;
        } else {
            self.irq_counter -= 1;
        }

        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_triggered = true;
        }
    }
}
