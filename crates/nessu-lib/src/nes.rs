use std::ops::DerefMut;

use crate::apu::Apu;
use crate::cartridge::Cartridge;
use crate::cpu::Cpu;
use crate::input::Button;
use crate::op::{into_op, op_size, AddressingMode, CpuOpEntry, OpKind};
use crate::ppu::{Ppu, DEFAULT_PALETTE};

pub struct Nes {
    pub(crate) cpu: Cpu,
    pub(crate) ppu: Ppu,
    pub(crate) apu: Apu,
    pub(crate) cart: Box<Cartridge>,

    counter: u128,
}

impl Nes {
    pub fn new() -> Self {
        let mut cart = Box::new(Cartridge::default());
        let cpu = Cpu::new();
        let ppu = Ppu::new(cart.deref_mut());
        let apu = Apu::new();

        Self {
            cpu,
            ppu,
            apu,
            cart,
            counter: 1,
        }
    }

    pub fn cartridge(&self) -> &Cartridge {
        &self.cart
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn cpu_mut(&mut self) -> &mut Cpu {
        &mut self.cpu
    }

    pub fn apu(&self) -> &Apu {
        &self.apu
    }

    pub fn ppu(&self) -> &Ppu {
        &self.ppu
    }

    pub fn ppu_mut(&mut self) -> &mut Ppu {
        &mut self.ppu
    }

    pub fn display_bytes(&self) -> &[u8] {
        &self.ppu.display
    }

    pub fn nametable_rgb_bytes(&mut self, nametable_idx: u8) -> Vec<u8> {
        let base_pattern_addr = self.ppu.background_pattern_table_address();

        let mut colors = vec![0; 0x3C000];
        let mut i = 0;

        for y0 in 0..30 {
            for x0 in 0..32 {
                let nt_byte = self
                    .ppu
                    .read_mem_u8(0x2000 + (0x400 * nametable_idx as u16) + i);
                i += 1;

                let tile_base_addr = base_pattern_addr + ((nt_byte as u16) << 4);

                let attr_addr = 0x23C0
                    | (0x400 * nametable_idx as usize)
                    | (((y0 as usize) >> 2) << 3)
                    | ((x0 as usize) >> 2);
                let mut attr_tile = self.ppu.read_mem_u8(attr_addr as u16);
                attr_tile >>= (((x0 & 0b10) >> 1) | (y0 & 0b10)) << 1;
                attr_tile &= 0b11;

                let attr_lo = (attr_tile & 0b01) * 0xFF;
                let attr_hi = ((attr_tile & 0b10) >> 1) * 0xFF;

                for y in 0..8 {
                    let tile_lo = self.ppu.read_mem_u8(tile_base_addr + y);
                    let tile_hi = self.ppu.read_mem_u8(tile_base_addr + y + 8);

                    for x in 0..8 {
                        let pixel_idx =
                            ((tile_lo >> (7 - x)) & 1) | (((tile_hi >> (7 - x)) & 1) << 1);

                        let palette_idx =
                            ((attr_lo >> (7 - x)) & 1) | (((attr_hi >> (7 - x)) & 1) << 1);

                        let color_idx = self
                            .ppu
                            .read_mem_u8(0x3F00 | ((palette_idx as u16) << 2) | pixel_idx as u16);

                        let display_idx =
                            (((y0 * 8 + y) as usize * 256) + (x0 * 8 + x) as usize) as usize * 4;

                        let color = DEFAULT_PALETTE[color_idx as usize & 0x3F];

                        colors[display_idx] = color.0;
                        colors[display_idx + 1] = color.1;
                        colors[display_idx + 2] = color.2;
                        colors[display_idx + 3] = 255;
                    }
                }
            }
        }

        colors
    }

    pub fn insert_cartridge(&mut self, cart: Cartridge) {
        self.cart = Box::new(cart);
        self.reset();
    }

    pub fn power(&mut self) {
        self.cpu = Cpu::new();
        self.ppu = Ppu::new(self.cart.deref_mut());
        self.apu = Apu::new();
        self.reset();
    }

    pub fn reset(&mut self) {
        self.ppu.reset(self.cart.deref_mut());
        self.cpu.reset();
        self.cpu.pc = self.cpu_read_mem(0xFFFC) as u16 | (self.cpu_read_mem(0xFFFD) as u16) << 8;
        self.apu.reset();
    }

    pub fn clock(&mut self) -> Result<(), String> {
        if !self.cart.is_valid() {
            return Ok(());
        }

        self.ppu.clock();

        if self.counter % 3 == 0 {
            Cpu::clock(self)?;
            self.apu.clock();
        }

        self.counter += 1;

        Ok(())
    }

    pub fn step_frame(&mut self) -> Result<(), String> {
        loop {
            self.clock()?;

            if self.ppu.frame_completed() {
                return Ok(());
            }
        }
    }

    pub fn step_instruction(&mut self) -> Result<(), String> {
        // clock until cpu instruction is started
        while !self.cpu.instruction_ongoing() {
            self.clock()?;
        }

        // complete the instruction
        while self.cpu.instruction_ongoing() {
            self.clock()?;
        }

        Ok(())
    }

    pub fn set_button_state_player1(&mut self, button: Button, state: bool) {
        self.cpu.set_button_state_player1(button, state);
    }

    pub fn _set_button_state_player2(&mut self, button: Button, state: bool) {
        self.cpu._set_button_state_player2(button, state);
    }

    pub fn cpu_read_mem(&mut self, addr: u16) -> u8 {
        Cpu::read_mem_u8(self, addr)
    }

    pub fn cpu_write_mem(&mut self, addr: u16, val: u8) {
        Cpu::write_mem_u8(self, addr, val)
    }

    pub fn ppu_read_mem(&mut self, addr: u16) -> u8 {
        self.ppu.read_mem_u8(addr)
    }

    pub fn ppu_write_mem(&mut self, addr: u16, val: u8) {
        self.ppu.write_mem_u8(addr, val)
    }

    pub fn cpu_disassembly(&mut self) -> Vec<CpuOpEntry> {
        let mut ops = Vec::new();

        let mut pc = 0x0000;

        loop {
            let op = self.cpu_op_at(pc);
            pc += op.size as u16;

            ops.push(op);

            if pc == 0xFFFF {
                break;
            }
        }

        ops
    }

    fn cpu_op_at(&mut self, addr: u16) -> CpuOpEntry {
        let opcode = self.cpu_read_mem(addr);
        let (kind, addr_mode) = match into_op(opcode) {
            Some((kind, addr_mode, _)) => (kind, addr_mode),
            None => {
                return CpuOpEntry {
                    addr,
                    opcode,
                    size: 1,
                    kind: OpKind::Invalid,
                    addr_mode: AddressingMode::Implied,
                    operands: [0, 0],
                }
            }
        };

        let size = op_size(addr_mode);

        if addr as u32 + size as u32 >= 0x10000 {
            return CpuOpEntry {
                addr,
                opcode,
                size: 1,
                kind: OpKind::Invalid,
                addr_mode: AddressingMode::Implied,
                operands: [0, 0],
            };
        }

        let mut operands: [u8; 2] = [0; 2];
        if size > 1 {
            operands[0] = self.cpu_read_mem(addr + 1);
        }
        if size > 2 {
            operands[1] = self.cpu_read_mem(addr + 2);
        }

        CpuOpEntry {
            addr,
            opcode,
            size,
            kind,
            addr_mode,
            operands,
        }
    }
}
