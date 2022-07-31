use std::collections::HashSet;
use std::fmt::Write;
use std::mem;
use std::ops::{Deref, DerefMut};

use crate::bitwise::{HasBits, HiLoBytes};
use crate::input::Button;
use crate::nes::Nes;
use crate::op::{into_op, op_size, to_asm, AccessMode, AddressingMode, OpKind};
use crate::rand_vec;

const STACK_START_ADDR: u16 = 0x0100;

/// Carry
const C: u8 = 0b0000_0001;
/// Zero
const Z: u8 = 0b0000_0010;
/// Interrupt disable
const I: u8 = 0b0000_0100;
/// Decimal
const D: u8 = 0b0000_1000;
/// Unused
const U: u8 = 0b0010_0000;
/// Unused 2
const B: u8 = 0b0001_0000;
/// Overflow
const O: u8 = 0b0100_0000;
/// Negative
const N: u8 = 0b1000_0000;

pub struct Cpu {
    /// Accumulator
    pub a: u8,
    /// X index
    pub x: u8,
    /// Y index
    pub y: u8,
    /// Program counter
    pub pc: u16,
    /// Stack pointer
    pub s: u8,
    /// Status register
    pub p: u8,

    pub branch_taken: bool,
    pub page_crossed: bool,

    internal_ram: Vec<u8>,

    pending_oamdma: OamDmaStatus,

    nmi_pending: Option<u8>,

    op_kind: Option<OpKind>,
    addressing_mode: AddressingMode,
    access_mode: AccessMode,

    /// Temporary address for operand memory access
    temp_addr: u16,
    /// Temporary value used by cpu operations
    temp_value: u16,

    /// Total elapsed cycles
    pub cycles: u128,

    /// Total cycles used by the last op
    pub prev_op_cycles: u8,
    /// Cycles used by the current op so far
    current_op_cycle: u8,
    /// Address of the last opcode
    op_start_addr: u16,

    breakpoints: HashSet<u16>,

    breakpoint_reached: bool,

    input_p1: u8,
    input_p2: u8,

    controller_p1: u8,
    controller_p2: u8,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            s: 0,
            p: 0x34,
            branch_taken: false,
            page_crossed: false,

            internal_ram: rand_vec![0x0800],
            pending_oamdma: OamDmaStatus {
                addr: 0,
                reading: false,
                byte: 0,
                cycle: 0,
                idx: 0xFF,
            },

            nmi_pending: None,

            op_kind: None,
            addressing_mode: AddressingMode::Implied,
            access_mode: AccessMode::Read,
            temp_addr: 0,
            temp_value: 0,
            cycles: 0,
            prev_op_cycles: 0,
            current_op_cycle: 0,
            op_start_addr: 0,
            breakpoints: HashSet::new(),
            breakpoint_reached: false,

            input_p1: 0,
            input_p2: 0,

            controller_p1: 0,
            controller_p2: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Cpu {
            a: self.a,
            x: self.x,
            y: self.y,
            p: self.p | 0x04,
            s: self.s.wrapping_sub(3),
            internal_ram: mem::take(&mut self.internal_ram),
            breakpoints: mem::take(&mut self.breakpoints),
            ..Cpu::new()
        }
    }

    pub fn set_button_state_player1(&mut self, button: Button, state: bool) {
        if state {
            self.input_p1 |= button as u8;
        } else {
            self.input_p1 &= !(button as u8);
        }
    }

    pub fn _set_button_state_player2(&mut self, button: Button, state: bool) {
        if state {
            self.input_p2 |= button as u8;
        } else {
            self.input_p2 &= !(button as u8);
        }
    }

    pub fn instruction_ongoing(&self) -> bool {
        self.op_kind.is_some()
    }

    pub fn clock(nes: &mut Nes) -> Result<(), String> {
        let ctx = CpuContext {
            nes,
            read_only: false,
        };

        ctx.clock()
    }

    pub fn read_mem_u8(nes: &mut Nes, addr: u16) -> u8 {
        CpuContext {
            nes,
            read_only: true,
        }
        .read_mem_u8(addr)
    }

    pub fn write_mem_u8(nes: &mut Nes, addr: u16, val: u8) {
        CpuContext {
            nes,
            read_only: false,
        }
        .write_mem_u8(addr, val)
    }

    pub fn is_breakpoint(&self, addr: u16) -> bool {
        self.breakpoints.contains(&addr)
    }

    pub fn set_breakpoint(&mut self, addr: u16) {
        self.breakpoints.insert(addr);
    }

    pub fn clear_breakpoint(&mut self, addr: u16) {
        self.breakpoints.remove(&addr);
    }

    pub fn toggle_breakpoint(&mut self, addr: u16) {
        if self.is_breakpoint(addr) {
            self.clear_breakpoint(addr);
        } else {
            self.set_breakpoint(addr);
        }
    }
}

struct CpuContext<'a> {
    nes: &'a mut Nes,
    read_only: bool,
}

impl<'a> Deref for CpuContext<'a> {
    type Target = Cpu;

    fn deref(&self) -> &Self::Target {
        &self.nes.cpu
    }
}

impl<'a> DerefMut for CpuContext<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.nes.cpu
    }
}

impl CpuContext<'_> {
    fn clock(mut self) -> Result<(), String> {
        if !self.instruction_ongoing() && self.is_breakpoint(self.pc) {
            self.breakpoint_reached = !self.breakpoint_reached;

            if self.breakpoint_reached {
                return Err("Breakpoint reached".to_string());
            }
        }
        self.breakpoint_reached = false;

        self.cycles += 1;

        if self.clock_oamdma() {
            // CPU is suspended while OAMDMA writing in progress.
            return Ok(());
        }

        self.current_op_cycle += 1;

        if self.op_kind.is_none() {
            return self.get_next_op();
        }

        match self.addressing_mode {
            AddressingMode::Relative => self.relative(),
            AddressingMode::Absolute => self.absolute(),
            AddressingMode::AbsoluteX | AddressingMode::AbsoluteY => self.absolute_indexed(),
            AddressingMode::ZeroPage => self.zero_page(),
            AddressingMode::ZeroPageX | AddressingMode::ZeroPageY => self.zero_page_indexed(),
            AddressingMode::Indirect => self.indirect(),
            AddressingMode::IndirectX => self.indirect_x(),
            AddressingMode::IndirectY => self.indirect_y(),
            AddressingMode::Accumulator => self.accumulator(),
            AddressingMode::Immediate => self.immediate(),
            AddressingMode::Implied => self.implied(),
        }

        if self.nes.ppu.nmi_triggered() {
            self.nmi_pending = Some(self.current_op_cycle);
        }

        Ok(())
    }

    fn clock_oamdma(&mut self) -> bool {
        if self.pending_oamdma.idx <= 0xFF {
            self.pending_oamdma.cycle += 1;

            if self.pending_oamdma.cycle > 2 {
                if self.pending_oamdma.reading {
                    self.pending_oamdma.byte = self.read_mem_u8(self.pending_oamdma.addr);
                } else {
                    self.write_mem_u8(0x2004, self.pending_oamdma.byte);

                    self.pending_oamdma.addr += 1;
                    self.pending_oamdma.idx += 1;
                }

                if self.pending_oamdma.reading || self.pending_oamdma.addr & 0xFF != 0 {
                    self.pending_oamdma.reading = !self.pending_oamdma.reading;
                }
            }

            true
        } else {
            false
        }
    }

    fn implied(&mut self) {
        if let Some(op_kind) = self.op_kind {
            match op_kind {
                OpKind::Sei => self.sei(),
                OpKind::Sec => self.sec(),
                OpKind::Cli => self.cli(),
                OpKind::Clc => self.clc(),
                OpKind::Sed => self.sed(),
                OpKind::Cld => self.cld(),
                OpKind::Clv => self.clv(),
                OpKind::Txs => self.txs(),
                OpKind::Tsx => self.tsx(),
                OpKind::Txa => self.txa(),
                OpKind::Tax => self.tax(),
                OpKind::Tay => self.tay(),
                OpKind::Tya => self.tya(),
                OpKind::Dex => self.dex(),
                OpKind::Dey => self.dey(),
                OpKind::Inx => self.inx(),
                OpKind::Iny => self.iny(),
                OpKind::Rti => self.rti(),
                OpKind::Rts => self.rts(),
                OpKind::Pla => self.pla(),
                OpKind::Plp => self.plp(),
                OpKind::Pha => self.pha(),
                OpKind::Php => self.php(),
                OpKind::Brk => self.brk(),
                OpKind::Nmi => self.nmi(),
                OpKind::Irq => self.irq(),
                OpKind::Nop => self.nop(),

                op_kind if self.current_op_cycle > 8 => panic!(
                    "No operation implemented for ({:?}) ({:?}) ({:?}) (op cycle {})",
                    op_kind, self.addressing_mode, self.access_mode, self.current_op_cycle
                ),
                _ => {}
            }
        }
    }

    fn sei(&mut self) {
        self.set_status_flag(I, true);
        self.complete_instruction();
    }

    fn clv(&mut self) {
        self.set_status_flag(O, false);
        self.complete_instruction();
    }

    fn cld(&mut self) {
        self.set_status_flag(D, false);
        self.complete_instruction();
    }

    fn sed(&mut self) {
        self.set_status_flag(D, true);
        self.complete_instruction();
    }

    fn clc(&mut self) {
        self.set_status_flag(C, false);
        self.complete_instruction();
    }

    fn cli(&mut self) {
        self.set_status_flag(I, false);
        self.complete_instruction();
    }

    fn sec(&mut self) {
        self.set_status_flag(C, true);
        self.complete_instruction();
    }

    fn nop(&mut self) {
        self.complete_instruction()
    }

    fn nmi(&mut self) {
        match self.current_op_cycle {
            2 => self.push_stack_u8(self.pc.high_u8()),
            3 => self.push_stack_u8(self.pc.low_u8()),
            4 => {
                self.set_status_flag(I, true);
                self.set_status_flag(U, true);
                self.set_status_flag(B, false);
                self.push_stack_u8(self.p);
            }
            5 => self.temp_value = self.read_mem_u8(0xFFFA) as u16,
            6 => self.temp_value |= (self.read_mem_u8(0xFFFB) as u16) << 8,
            7 => {
                self.pc = self.temp_value;
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn irq(&mut self) {
        match self.current_op_cycle {
            2 => self.push_stack_u8(self.pc.high_u8()),
            3 => self.push_stack_u8(self.pc.low_u8()),
            4 => {
                self.set_status_flag(I, true);
                self.set_status_flag(B, false);
                self.set_status_flag(U, true);
                self.push_stack_u8(self.p);
            }
            5 => self.temp_value = self.read_mem_u8(0xFFFE) as u16,
            6 => self.temp_value |= (self.read_mem_u8(0xFFFF) as u16) << 8,
            7 => {
                self.pc = self.temp_value;
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn brk(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_next_pc_u8();
                self.increment_pc();
            }
            3 => self.push_stack_u8(self.pc.high_u8()),
            4 => self.push_stack_u8(self.pc.low_u8()),
            5 => {
                self.set_status_flag(B, true);
                self.set_status_flag(I, true);
                self.push_stack_u8(self.p);
            }
            6 => self.pc = self.read_mem_u8(0xFFFE) as u16,
            7 => {
                self.pc |= (self.read_mem_u8(0xFFFF) as u16) << 8;
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn php(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_next_pc_u8();
            }
            3 => {
                self.push_stack_u8(self.p | B | U);
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn pha(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_next_pc_u8();
            }
            3 => {
                self.push_stack_u8(self.a);
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn plp(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_next_pc_u8();
            }
            3 => {
                self.increment_stack_pointer();
            }
            4 => {
                self.p = self.read_stack_u8() | U;
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn pla(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_next_pc_u8();
            }
            3 => {
                self.increment_stack_pointer();
            }
            4 => {
                self.a = self.read_stack_u8();
                self.set_status_flag(Z, self.a == 0);
                self.set_status_flag(N, self.a.has_bits(0b1000_0000));
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn rti(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_next_pc_u8();
            }
            3 => {
                self.increment_stack_pointer();
            }
            4 => {
                self.p = self.read_stack_u8() & !B & !U;
                self.increment_stack_pointer();
            }
            5 => {
                self.pc = self.read_stack_u8() as u16;
                self.increment_stack_pointer();
            }
            6 => {
                self.pc |= (self.read_stack_u8() as u16) << 8;
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn rts(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_next_pc_u8();
            }
            3 => {
                self.increment_stack_pointer();
            }
            4 => {
                self.pc = self.read_stack_u8() as u16;
                self.increment_stack_pointer();
            }
            5 => {
                self.pc |= (self.read_stack_u8() as u16) << 8;
            }
            6 => {
                self.increment_pc();
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn immediate(&mut self) {
        self.temp_value = self.read_next_pc_u8() as u16;
        self.increment_pc();

        self.do_read_operation();
        self.complete_instruction();
    }

    fn accumulator(&mut self) {
        self.do_modify_operation();
        self.complete_instruction();
    }

    fn indirect_y(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_addr_low();
                self.increment_pc();
            }
            3 => {
                self.read_from_effective_addr_low();
            }
            4 => {
                self.read_from_effective_addr_high();

                let prev_addr = self.temp_addr;
                self.temp_addr = self.temp_addr.wrapping_add(self.y as u16);

                if self.temp_addr.high_u8() != prev_addr.high_u8() {
                    self.page_crossed = true;
                }
            }
            5 => {
                self.read_from_effective_addr_low();

                if !self.page_crossed && self.access_mode == AccessMode::Read {
                    self.do_read_operation();
                    self.complete_instruction();
                }
            }
            6 => {
                if self.page_crossed && self.access_mode == AccessMode::Read {
                    self.read_from_effective_addr_low();
                    self.do_read_operation();
                    self.complete_instruction();
                } else if self.access_mode == AccessMode::Write {
                    self.do_write_operation();
                    self.complete_instruction();
                } else if self.access_mode == AccessMode::ReadModifyWrite {
                    self.read_from_effective_addr_low();
                }
            }
            7 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the value back to effective address and do the operation on it
                self.write_to_effective_addr();
                self.do_modify_operation();
            }
            8 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the new value to effective address
                self.write_to_effective_addr();
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn indirect_x(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_addr_low();
                self.increment_pc();
            }
            3 => {
                self.read_from_effective_addr_low();
                self.temp_addr = self.temp_addr.wrapping_add(self.x as u16) & 0xFF;
            }
            4 => {
                self.read_from_effective_addr_low();
            }
            5 => {
                self.read_from_effective_addr_high();
            }
            6 => match self.access_mode {
                AccessMode::Read => {
                    self.read_from_effective_addr_low();
                    self.do_read_operation();
                    self.complete_instruction();
                }
                AccessMode::ReadModifyWrite => {
                    self.read_from_effective_addr_low();
                }
                AccessMode::Write => {
                    self.do_write_operation();
                    self.complete_instruction();
                }
            },
            7 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the value back to effective address and do the operation on it
                self.write_to_effective_addr();
                self.do_modify_operation();
            }
            8 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the new value to effective address
                self.write_to_effective_addr();
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn indirect(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_addr_low();
                self.increment_pc();
            }
            3 => {
                self.read_addr_high();
                self.increment_pc();
            }
            4 => {
                self.read_from_effective_addr_low();
            }
            5 => {
                self.read_from_effective_addr_high();

                if self.op_kind == Some(OpKind::Jmp) {
                    self.pc = self.temp_addr;
                    self.complete_instruction();
                }
            }
            _ => {}
        }
    }

    fn zero_page_indexed(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_addr_low();
                self.increment_pc();
            }
            3 => {
                self.read_from_effective_addr_low();
                if self.addressing_mode == AddressingMode::ZeroPageX {
                    self.temp_addr = self.temp_addr.low_u8().wrapping_add(self.x) as u16;
                } else {
                    self.temp_addr = self.temp_addr.low_u8().wrapping_add(self.y) as u16;
                }
            }
            4 => match self.access_mode {
                AccessMode::Read => {
                    self.read_from_effective_addr_low();
                    self.do_read_operation();
                    self.complete_instruction();
                }
                AccessMode::ReadModifyWrite => {
                    self.read_from_effective_addr_low();
                }
                AccessMode::Write => {
                    self.do_write_operation();
                    self.complete_instruction();
                }
            },
            5 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the value back to effective address and do the operation on it
                self.write_to_effective_addr();
                self.do_modify_operation();
            }
            6 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the new value to effective address
                self.write_to_effective_addr();
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn zero_page(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_addr_low();
                self.increment_pc();
            }
            3 => match self.access_mode {
                AccessMode::Read => {
                    self.read_from_effective_addr_low();
                    self.do_read_operation();
                    self.complete_instruction();
                }
                AccessMode::ReadModifyWrite => {
                    self.read_from_effective_addr_low();
                }
                AccessMode::Write => {
                    self.do_write_operation();
                    self.complete_instruction();
                }
            },
            4 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the value back to effective address and do the operation on it
                self.write_to_effective_addr();
                self.do_modify_operation();
            }
            5 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the new value to effective address
                self.write_to_effective_addr();
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn absolute_indexed(&mut self) {
        match self.current_op_cycle {
            2 => {
                self.read_addr_low();
                self.increment_pc();
            }
            3 => {
                self.read_addr_high();
                self.increment_pc();
                let prev_addr = self.temp_addr;
                if self.addressing_mode == AddressingMode::AbsoluteX {
                    self.temp_addr = self.temp_addr.wrapping_add(self.x as u16);
                } else {
                    self.temp_addr = self.temp_addr.wrapping_add(self.y as u16);
                }
                if prev_addr.high_u8() != self.temp_addr.high_u8() {
                    self.page_crossed = true;
                }
            }
            4 => {
                if !self.page_crossed && self.access_mode == AccessMode::Read {
                    self.read_from_effective_addr_low();
                    self.do_read_operation();
                    self.complete_instruction();
                } else {
                    self.read_from_effective_addr_low();
                }
            }
            5 => match self.access_mode {
                AccessMode::Read => {
                    self.read_from_effective_addr_low();
                    self.do_read_operation();
                    self.complete_instruction();
                }
                AccessMode::ReadModifyWrite => {
                    self.read_from_effective_addr_low();
                }
                AccessMode::Write => {
                    self.do_write_operation();
                    self.complete_instruction();
                }
            },
            6 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the value back to effective address and do the operation on it
                self.write_to_effective_addr();
                self.do_modify_operation();
            }
            7 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the new value to effective address
                self.write_to_effective_addr();
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn absolute(&mut self) {
        if self.current_op_cycle >= 3 && self.op_kind == Some(OpKind::Jsr) {
            return self.jsr();
        }

        match self.current_op_cycle {
            2 => {
                self.read_addr_low();
                self.increment_pc();
            }
            3 => {
                self.read_addr_high();
                self.increment_pc();

                if self.op_kind == Some(OpKind::Jmp) {
                    self.pc = self.temp_addr;
                    self.complete_instruction();
                }
            }
            4 => match self.access_mode {
                AccessMode::Read => {
                    self.read_from_effective_addr_low();
                    self.do_read_operation();
                    self.complete_instruction();
                }
                AccessMode::ReadModifyWrite => {
                    self.read_from_effective_addr_low();
                }
                AccessMode::Write => {
                    self.do_write_operation();
                    self.complete_instruction();
                }
            },
            5 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the value back to effective address and do the operation on it
                self.write_to_effective_addr();
                self.do_modify_operation();
            }
            6 if self.access_mode == AccessMode::ReadModifyWrite => {
                // write the new value to effective address
                self.write_to_effective_addr();
                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn jsr(&mut self) {
        match self.current_op_cycle {
            3 => {
                // ??? internal operation (predecrement S?)
            }
            4 => {
                self.push_stack_u8(self.pc.high_u8());
            }
            5 => {
                self.push_stack_u8(self.pc.low_u8());
            }
            6 => {
                self.read_addr_high();
                self.pc = self.temp_addr;
                self.complete_instruction()
            }
            _ => {}
        }
    }

    fn relative(&mut self) {
        match self.current_op_cycle {
            2 => {
                let offset = self.read_mem_u8(self.pc) as i8 as u16;
                self.pc += 1;
                self.temp_addr = self.pc.wrapping_add(offset);
            }
            3 => {
                let _ = self.read_mem_u8(self.pc) as i8 as u16;
            }
            _ => {}
        }

        match self.op_kind.unwrap() {
            OpKind::Bcc => self.bcc(),
            OpKind::Bcs => self.bcs(),
            OpKind::Beq => self.beq(),
            OpKind::Bmi => self.bmi(),
            OpKind::Bne => self.bne(),
            OpKind::Bpl => self.bpl(),
            OpKind::Bvc => self.bvc(),
            OpKind::Bvs => self.bvs(),
            kind => panic!("Invalid relative op: {:?}", kind),
        }
    }

    fn complete_instruction(&mut self) {
        if cfg!(feature = "logging") && log::log_enabled!(log::Level::Debug) {
            self.log_op_asm(self.op_start_addr, self.current_op_cycle);
        }

        self.op_kind = None;
        self.branch_taken = false;
        self.page_crossed = false;

        self.prev_op_cycles = self.current_op_cycle;
        self.current_op_cycle = 0;

        if self.nmi_pending.is_some() {
            self.nmi_pending = None;
            self.op_kind = Some(OpKind::Nmi);
            self.addressing_mode = AddressingMode::Implied;
        } else if self.nes.cart.irq_triggered() && !self.is_interrupt_disable_flag_set() {
            self.op_kind = Some(OpKind::Irq);
            self.addressing_mode = AddressingMode::Implied;
        }
    }

    fn effective_cpu_addr(&self, addr: u16) -> u16 {
        match addr {
            0x0800..=0x1FFF => addr & 0x07FF,
            0x2008..=0x3FFF => addr & 0x2007,
            _ => addr,
        }
    }

    fn read_mem_u8(&mut self, addr: u16) -> u8 {
        let addr = self.effective_cpu_addr(addr) as usize;

        match addr {
            0x0000..=0x7FF => self.internal_ram[addr],
            0x2000 => self.read_ppu_open_bus(),
            0x2001 => self.read_ppu_open_bus(),
            0x2002 => self.read_ppu_status(),
            0x2003 => self.read_ppu_open_bus(),
            0x2004 => self.read_oam_data(),
            0x2005 => self.read_ppu_open_bus(),
            0x2006 => self.read_ppu_open_bus(),
            0x4000..=0x4007 => self.read_ppu_open_bus(),
            0x2007 => self.read_ppu_data(),
            0x4015 => 0,
            0x4016 => self.read_controller_p1(),
            0x4017 => self.read_controller_p2(),
            _ => self.nes.cart.cpu_read_u8(addr),
        }
    }

    fn read_ppu_status(&mut self) -> u8 {
        self.nes.ppu.read_ppu_status(self.read_only)
    }

    fn read_ppu_open_bus(&mut self) -> u8 {
        self.nes.ppu.open_bus
    }

    fn read_ppu_data(&mut self) -> u8 {
        self.nes.ppu.read_ppu_data(self.read_only)
    }

    fn read_oam_data(&mut self) -> u8 {
        // Bits 2-4 of sprite attributes should always be clear when read

        // TODO reads during vertical or forced blanking return the value from OAM at that address but do not increment.
        // TODO Reading OAMDATA while the PPU is rendering will expose internal OAM accesses during sprite evaluation and loading; Micro Machines does this.

        let val = self.nes.ppu.primary_oam[self.nes.ppu.oam_addr as usize] & 0xE3;
        self.nes.ppu.write_open_bus(val, false);
        val
    }

    fn read_controller_p1(&mut self) -> u8 {
        let val = self.controller_p1 >> 7;
        if !self.read_only {
            self.controller_p1 <<= 1;
        }
        val
    }

    fn read_controller_p2(&mut self) -> u8 {
        let val = self.controller_p2 >> 7;
        if !self.read_only {
            self.controller_p2 <<= 1;
        }
        val
    }

    fn read_mem_u16(&mut self, addr: u16) -> u16 {
        self.read_mem_u8(addr) as u16 | ((self.read_mem_u8(addr + 1) as u16) << 8)
    }

    fn write_mem_u8(&mut self, addr: u16, val: u8) {
        let addr = self.effective_cpu_addr(addr) as usize;

        match addr {
            0x0000..=0x7FF => self.internal_ram[addr] = val,

            0x2001 => self.write_ppu_mask(val),
            0x2000 => self.write_ppu_ctrl(val),
            0x2002 => self.nes.ppu.write_open_bus(val, true),
            0x2003 => self.nes.ppu.write_oam_addr(val),
            0x2004 => self.nes.ppu.write_oam_data(val),
            0x2005 => self.nes.ppu.write_ppu_scroll(val),
            0x2006 => self.nes.ppu.write_ppu_addr(val),
            0x2007 => self.nes.ppu.write_vram(val),

            0x4000..=0x4013 => {}

            0x4014 => self.write_oamdma(val),

            0x4015 => {}

            0x4016 => {
                self.controller_p1 = self.input_p1;
            }
            0x4017 => {
                self.controller_p2 = self.input_p2;
            }

            _ => {
                let cycles = self.cycles;
                self.nes.cart.cpu_write_u8(addr, val, cycles)
            }
        }
    }

    fn write_ppu_mask(&mut self, val: u8) {
        self.nes.ppu.write_ppu_mask(val);
    }

    fn write_ppu_ctrl(&mut self, val: u8) {
        self.nes.ppu.write_ppu_ctrl(val);
    }

    fn write_oamdma(&mut self, val: u8) {
        self.pending_oamdma = OamDmaStatus {
            addr: (val as u16) << 8,
            reading: true,
            byte: 0,
            idx: 0,
            cycle: (self.cycles & 1) as u16,
        };
    }

    fn log_op_asm(&mut self, addr: u16, cycles: u8) {
        if self.op_kind == Some(OpKind::Nmi) {
            log::debug!("NMI");
            return;
        }

        let opcode = self.read_mem_u8(addr);
        let (op_kind, addr_mode, _acc_mode) = into_op(opcode).ok_or(opcode).unwrap();
        let op_size = op_size(addr_mode);

        let asm = match op_size {
            2 => to_asm(op_kind, addr_mode, self.read_mem_u8(addr + 1) as u16),
            3 => to_asm(op_kind, addr_mode, self.read_mem_u16(addr + 1)),
            _ => to_asm(op_kind, addr_mode, 0),
        };

        let mut msg = String::new();
        write!(msg, "${:04X}: {:02X} ", addr, opcode).unwrap();

        match op_size {
            2 => write!(msg, "{:02X}    | ", self.read_mem_u8(addr + 1)),
            3 => write!(
                msg,
                "{:02X} {:02X} | ",
                self.read_mem_u8(addr + 1),
                self.read_mem_u8(addr + 2)
            ),
            _ => write!(msg, "      | "),
        }
        .unwrap();

        write!(msg, "{} ({} cycles)", asm, cycles).unwrap();

        if self.branch_taken {
            write!(msg, " (branch taken)").unwrap();
        }

        if self.page_crossed {
            write!(msg, " (page crossed)").unwrap();
        }

        log::debug!("{}", msg);
    }

    fn read_next_pc_u8(&mut self) -> u8 {
        self.read_mem_u8(self.pc)
    }

    fn increment_pc(&mut self) {
        self.pc += 1;
    }

    fn read_addr_low(&mut self) {
        self.temp_addr = self.read_next_pc_u8() as u16;
    }

    fn read_addr_high(&mut self) {
        self.temp_addr |= (self.read_next_pc_u8() as u16) << 8;
    }

    fn read_temp_value_high(&mut self) {
        let addr = match self.addressing_mode {
            AddressingMode::IndirectX | AddressingMode::IndirectY => {
                // Keep address within zero-page
                (self.temp_addr + 1) & 0xFF
            }
            AddressingMode::Indirect if (self.temp_addr & 0xFF) == 0xFF => {
                // Page boundary hardware bug
                self.temp_addr & 0xFF00
            }
            _ => self.temp_addr + 1,
        };
        self.temp_value |= (self.read_mem_u8(addr) as u16) << 8;
    }

    fn read_from_effective_addr_low(&mut self) {
        self.temp_value = self.read_mem_u8(self.temp_addr) as u16;
    }

    fn write_to_effective_addr(&mut self) {
        self.write_mem_u8(self.temp_addr, self.temp_value.low_u8());
    }

    fn do_read_operation(&mut self) {
        match self.op_kind.unwrap() {
            OpKind::Dop => self.nop(),
            OpKind::Ldx => self.ldx(),
            OpKind::Ldy => self.ldy(),
            OpKind::Lda => self.lda(),
            OpKind::Cpx => self.cpx(),
            OpKind::Cpy => self.cpy(),
            OpKind::Cmp => self.cmp(),
            OpKind::Eor => self.eor(),
            OpKind::And => self.and(),
            OpKind::Adc => self.adc(),
            OpKind::Ora => self.ora(),
            OpKind::Bit => self.bit(),
            OpKind::Sbc => self.sbc(),
            OpKind::Aac => self.aac(),
            OpKind::Asr => self.asr(),

            op_kind => panic!(
                "No read operation implemented for {:?} {:?} {:?}",
                op_kind, self.addressing_mode, self.access_mode
            ),
        }
    }

    fn do_write_operation(&mut self) {
        match self.op_kind.unwrap() {
            OpKind::Sta => self.sta(),
            OpKind::Stx => self.stx(),
            OpKind::Sty => self.sty(),
            op_kind => panic!(
                "No write operation implemented for {:?} {:?} {:?}",
                op_kind, self.addressing_mode, self.access_mode
            ),
        }
    }

    fn do_modify_operation(&mut self) {
        match self.op_kind.unwrap() {
            OpKind::Inc => self.inc(),
            OpKind::Dec => self.dec(),
            OpKind::Lsr => self.lsr(),
            OpKind::Asl => self.asl(),
            OpKind::Ror => self.ror(),
            OpKind::Rol => self.rol(),

            op_kind => panic!(
                "No modify operation implemented for {:?} {:?} {:?}",
                op_kind, self.addressing_mode, self.access_mode
            ),
        }
    }

    fn read_from_effective_addr_high(&mut self) {
        self.read_temp_value_high();
        self.temp_addr = self.temp_value;
    }

    fn get_next_op(&mut self) -> Result<(), String> {
        self.op_start_addr = self.pc;
        let opcode = self.read_next_pc_u8();
        self.increment_pc();

        let (op_kind, addressing_mode, access_mode) = into_op(opcode).ok_or_else(|| {
            format!(
                "Unknown opcode at ${:04X}: ${:02X}",
                self.op_start_addr, opcode
            )
        })?;

        self.op_kind = Some(op_kind);
        self.addressing_mode = addressing_mode;
        self.access_mode = access_mode;

        Ok(())
    }

    fn lsr(&mut self) {
        if self.addressing_mode == AddressingMode::Accumulator {
            self.set_status_flag(C, self.a.has_bits(0x01));

            self.a >>= 1;
            self.set_status_flag(Z, self.a == 0);
            self.set_status_flag(N, false);
        } else {
            self.set_status_flag(C, self.temp_value.low_u8().has_bits(0x01));

            self.temp_value >>= 1;
            self.set_status_flag(Z, self.temp_value.low_u8() == 0);
            self.set_status_flag(N, false);
        }
    }

    fn asl(&mut self) {
        if self.addressing_mode == AddressingMode::Accumulator {
            self.set_status_flag(C, self.a.has_bits(0x80));

            self.a <<= 1;
            self.set_status_flag(Z, self.a == 0);
            self.set_status_flag(N, self.a.has_bits(0x80));
        } else {
            self.set_status_flag(C, self.temp_value.has_bits(0x80));

            self.temp_value <<= 1;
            self.set_status_flag(Z, self.temp_value.low_u8() == 0);
            self.set_status_flag(N, self.temp_value.has_bits(0x80));
        }
    }

    fn is_zero_flag_set(&self) -> bool {
        self.p & Z == Z
    }

    fn is_carry_flag_set(&self) -> bool {
        self.p & C == C
    }

    fn is_overflow_flag_set(&self) -> bool {
        self.p & O == O
    }

    fn is_negative_flag_set(&self) -> bool {
        self.p & N == N
    }

    fn is_interrupt_disable_flag_set(&self) -> bool {
        self.p & I == I
    }

    fn txs(&mut self) {
        self.s = self.x;
        self.complete_instruction();
    }

    fn tsx(&mut self) {
        self.x = self.s;

        self.set_status_flag(Z, self.x == 0);
        self.set_status_flag(N, self.x.has_bits(0x80));
        self.complete_instruction();
    }

    fn txa(&mut self) {
        self.a = self.x;

        self.set_status_flag(Z, self.a == 0);
        self.set_status_flag(N, self.a.has_bits(0x80));
        self.complete_instruction();
    }

    fn tya(&mut self) {
        self.a = self.y;

        self.set_status_flag(Z, self.a == 0);
        self.set_status_flag(N, self.a.has_bits(0x80));
        self.complete_instruction();
    }

    fn tax(&mut self) {
        self.x = self.a;

        self.set_status_flag(Z, self.x == 0);
        self.set_status_flag(N, self.x.has_bits(0x80));
        self.complete_instruction();
    }

    fn tay(&mut self) {
        self.y = self.a;

        self.set_status_flag(Z, self.y == 0);
        self.set_status_flag(N, self.y.has_bits(0x80));
        self.complete_instruction();
    }

    fn bpl(&mut self) {
        self.branch(!self.is_negative_flag_set())
    }

    fn bmi(&mut self) {
        self.branch(self.is_negative_flag_set())
    }

    fn bcc(&mut self) {
        self.branch(!self.is_carry_flag_set())
    }

    fn bcs(&mut self) {
        self.branch(self.is_carry_flag_set())
    }

    fn bne(&mut self) {
        self.branch(!self.is_zero_flag_set())
    }

    fn beq(&mut self) {
        self.branch(self.is_zero_flag_set())
    }

    fn bvc(&mut self) {
        self.branch(!self.is_overflow_flag_set())
    }

    fn bvs(&mut self) {
        self.branch(self.is_overflow_flag_set())
    }

    fn branch(&mut self, branch_taken: bool) {
        match self.current_op_cycle {
            2 => {
                self.branch_taken = branch_taken;
                if !branch_taken {
                    self.complete_instruction();
                }
            }
            3 => {
                if self.temp_addr.high_u8() != self.pc.high_u8() {
                    self.page_crossed = true;
                }
                self.pc = self.temp_addr;

                if !self.page_crossed {
                    self.complete_instruction();
                }
            }
            4 => {
                // Fetch opcode of next instruction
                let _ = self.read_mem_u8(self.pc);

                self.complete_instruction();
            }
            _ => {}
        }
    }

    fn lda(&mut self) {
        self.a = self.temp_value.low_u8();

        self.set_status_flag(Z, self.a == 0);
        self.set_status_flag(N, self.a.has_bits(0x80));
    }

    fn ldx(&mut self) {
        self.x = self.temp_value.low_u8();

        self.set_status_flag(Z, self.x == 0);
        self.set_status_flag(N, self.x.has_bits(0x80));
    }

    fn ldy(&mut self) {
        self.y = self.temp_value.low_u8();

        self.set_status_flag(Z, self.y == 0);
        self.set_status_flag(N, self.y.has_bits(0x80));
    }

    fn inc(&mut self) {
        self.temp_value = self.temp_value.low_u8().wrapping_add(1) as u16;

        self.set_status_flag(Z, self.temp_value.low_u8() == 0);
        self.set_status_flag(N, self.temp_value.has_bits(0x80));
    }

    fn dec(&mut self) {
        self.temp_value = self.temp_value.low_u8().wrapping_sub(1) as u16;

        self.set_status_flag(Z, self.temp_value.low_u8() == 0);
        self.set_status_flag(N, self.temp_value.has_bits(0x80));
    }

    fn dex(&mut self) {
        self.x = self.x.wrapping_sub(1);

        self.set_status_flag(Z, self.x == 0);
        self.set_status_flag(N, self.x.has_bits(0x80));
        self.complete_instruction();
    }

    fn dey(&mut self) {
        self.y = self.y.wrapping_sub(1);

        self.set_status_flag(Z, self.y == 0);
        self.set_status_flag(N, self.y.has_bits(0x80));
        self.complete_instruction();
    }

    fn inx(&mut self) {
        self.x = self.x.wrapping_add(1);

        self.set_status_flag(Z, self.x == 0);
        self.set_status_flag(N, self.x.has_bits(0x80));
        self.complete_instruction();
    }

    fn iny(&mut self) {
        self.y = self.y.wrapping_add(1);

        self.set_status_flag(Z, self.y == 0);
        self.set_status_flag(N, self.y.has_bits(0x80));

        self.complete_instruction();
    }

    fn cpx(&mut self) {
        self.compare(self.x, self.temp_value.low_u8());
    }

    fn cpy(&mut self) {
        self.compare(self.y, self.temp_value.low_u8());
    }

    fn cmp(&mut self) {
        self.compare(self.a, self.temp_value.low_u8());
    }

    fn compare(&mut self, first: u8, second: u8) {
        let sub = first.wrapping_sub(second);

        self.set_status_flag(C, first >= second);
        self.set_status_flag(Z, sub == 0);
        self.set_status_flag(N, sub.has_bits(0x80));
    }

    fn bit(&mut self) {
        let val = self.temp_value.low_u8();

        self.set_status_flag(Z, val & self.a == 0);
        self.set_status_flag(O, val.has_bits(0x40));
        self.set_status_flag(N, val.has_bits(0x80));
    }

    fn and(&mut self) {
        self.a &= self.temp_value.low_u8();

        self.set_status_flag(Z, self.a == 0);
        self.set_status_flag(N, self.a.has_bits(0x80));
    }

    fn ora(&mut self) {
        self.a |= self.temp_value.low_u8();

        self.set_status_flag(Z, self.a == 0);
        self.set_status_flag(N, self.a.has_bits(0x80));
    }

    fn sbc(&mut self) {
        let (sum, carry1) = self.a.overflowing_add(!self.temp_value.low_u8());
        let (sum, carry2) = sum.overflowing_add(self.is_carry_flag_set() as u8);
        let carry = carry1 || carry2;

        let overflow = (!(self.a ^ !self.temp_value.low_u8()) & (self.a ^ sum)).has_bits(0x80);
        self.set_status_flag(O, overflow);
        self.set_status_flag(C, carry);
        self.set_status_flag(Z, sum == 0);
        self.set_status_flag(N, sum.has_bits(0x80));

        self.a = sum;
    }

    fn adc(&mut self) {
        let (sum, carry1) = self.a.overflowing_add(self.temp_value.low_u8());
        let (sum, carry2) = sum.overflowing_add(self.is_carry_flag_set() as u8);
        let carry = carry1 || carry2;

        let overflow = (!(self.a ^ self.temp_value.low_u8()) & (self.a ^ sum)).has_bits(0x80);
        self.set_status_flag(O, overflow);
        self.set_status_flag(C, carry);
        self.set_status_flag(Z, sum == 0);
        self.set_status_flag(N, sum.has_bits(0x80));

        self.a = sum;
    }

    fn eor(&mut self) {
        self.a ^= self.temp_value.low_u8();

        self.set_status_flag(Z, self.a == 0);
        self.set_status_flag(N, self.a.has_bits(0x80));
    }

    fn rol(&mut self) {
        if self.addressing_mode == AddressingMode::Accumulator {
            let carry = self.is_carry_flag_set() as u8;

            self.set_status_flag(C, self.a.has_bits(0x80));

            self.a = (self.a << 1) | carry;

            self.set_status_flag(Z, self.a == 0);
            self.set_status_flag(N, self.a.has_bits(0x80));
        } else {
            let carry = self.is_carry_flag_set() as u8;

            self.set_status_flag(C, self.temp_value.has_bits(0x80));

            self.temp_value = ((self.temp_value.low_u8() << 1) | carry) as u16;

            self.set_status_flag(Z, self.temp_value.low_u8() == 0);
            self.set_status_flag(N, self.temp_value.has_bits(0x80));
        }
    }

    fn ror(&mut self) {
        if self.addressing_mode == AddressingMode::Accumulator {
            let carry = self.is_carry_flag_set() as u8;
            self.set_status_flag(C, self.a.has_bits(0x01));

            self.a = (self.a >> 1) | (carry << 7);

            self.set_status_flag(Z, self.a == 0);
            self.set_status_flag(N, self.a.has_bits(0x80));
        } else {
            let carry = self.is_carry_flag_set() as u8;
            self.set_status_flag(C, self.temp_value.has_bits(0x01));

            self.temp_value = ((self.temp_value.low_u8() >> 1) | (carry << 7)) as u16;

            self.set_status_flag(Z, self.temp_value.low_u8() == 0);
            self.set_status_flag(N, self.temp_value.has_bits(0x80));
        }
    }

    fn sta(&mut self) {
        self.write_mem_u8(self.temp_addr, self.a);
    }

    fn stx(&mut self) {
        self.write_mem_u8(self.temp_addr, self.x);
    }

    fn sty(&mut self) {
        self.write_mem_u8(self.temp_addr, self.y);
    }

    fn aac(&mut self) {
        self.a &= self.temp_value.low_u8();

        self.set_status_flag(Z, self.a == 0);
        self.set_status_flag(N, self.a.has_bits(0x80));
        self.set_status_flag(C, self.a.has_bits(0x80));
    }

    fn asr(&mut self) {
        self.a &= self.temp_value.low_u8();

        self.set_status_flag(C, self.a.has_bits(0b1));
        self.a >>= 1;

        self.set_status_flag(Z, self.a == 0);
        self.set_status_flag(N, self.a.has_bits(0x80));
    }

    /// Write a value to stack and decrement the stack pointer.
    fn push_stack_u8(&mut self, val: u8) {
        self.write_stack_u8(val);
        self.decrement_stack_pointer();
    }

    /// Increment the stack pointer and read a value from the stack.
    fn _pop_stack_u8(&mut self) -> u8 {
        self.increment_stack_pointer();
        self.read_stack_u8()
    }

    /// Write a value to stack at the current stack address.
    fn write_stack_u8(&mut self, val: u8) {
        self.write_mem_u8(STACK_START_ADDR + self.s as u16, val);
    }

    /// Read a value from stack at the current stack address.
    fn read_stack_u8(&mut self) -> u8 {
        self.read_mem_u8(STACK_START_ADDR + self.s as u16)
    }

    fn increment_stack_pointer(&mut self) {
        self.s = self.s.wrapping_add(1);
    }

    fn decrement_stack_pointer(&mut self) {
        self.s = self.s.wrapping_sub(1);
    }

    fn set_status_flag(&mut self, bit: u8, state: bool) {
        if state {
            self.p |= bit;
        } else {
            self.p &= !bit;
        }
    }
}

struct OamDmaStatus {
    addr: u16,
    reading: bool,
    byte: u8,
    cycle: u16,
    idx: u16,
}
