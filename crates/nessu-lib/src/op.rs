#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum AddressingMode {
    Implied,
    Accumulator,
    Immediate,
    Relative,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Indirect,
    IndirectX,
    IndirectY,
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum AccessMode {
    Read,
    Write,
    ReadModifyWrite,
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum OpKind {
    /// Add with Carry
    Adc,
    /// Logical AND
    And,
    /// Arithmetic Shift Left
    Asl,
    /// Branch if Carry Clear
    Bcc,
    /// Branch if Carry Set
    Bcs,
    /// Branch if Equal
    Beq,
    /// Branch if Minus
    Bmi,
    /// Branch if Not Equal
    Bne,
    /// Branch if Plus
    Bpl,
    /// Branch if Overflow Clear
    Bvc,
    /// Branch if Overflow Set
    Bvs,
    /// Bit Test
    Bit,
    /// Force Interrupt
    Brk,
    /// Clear Carry Flag
    Clc,
    /// Clear Decimal Mode
    Cld,
    /// Clear Interrupt Disable
    Cli,
    /// Clear Overflow Flag
    Clv,
    /// Compare
    Cmp,
    /// Compare X Register
    Cpx,
    /// Compare Y Register
    Cpy,
    /// Decrement Memory
    Dec,
    /// Decrement X Register
    Dex,
    /// Decrement Y Register
    Dey,
    /// Exclusive OR
    Eor,
    /// Increment Memory
    Inc,
    /// Increment X Register
    Inx,
    /// Increment Y Register
    Iny,
    /// Jump
    Jmp,
    /// Jump to Subroutine
    Jsr,
    /// Load Accumulator
    Lda,
    /// Load X Register
    Ldx,
    /// Load Y Register
    Ldy,
    /// Logical Shift Right
    Lsr,
    /// No Operation
    Nop,
    /// Logical OR
    Ora,
    /// Push Accumulator
    Pha,
    /// Push Processor Status
    Php,
    /// Pull Accumulator
    Pla,
    /// Pull Processor Status
    Plp,
    /// Rotate Left
    Rol,
    /// Rotate Right
    Ror,
    /// Return from Interrupt
    Rti,
    /// Return from Subroutine
    Rts,
    /// Subtract with Carry
    Sbc,
    /// Set Carry Flag
    Sec,
    /// Set Decimal Flag
    Sed,
    /// Set Interrupt Disable
    Sei,
    /// Store Accumulator
    Sta,
    /// Store X Register
    Stx,
    /// Store Y Register
    Sty,
    /// Transfer Accumulator to X
    Tax,
    /// Transfer Accumulator to Y
    Tay,
    /// Transfer Stack Pointer to X
    Tsx,
    /// Transfer X to Accumulator
    Txa,
    /// Transfer X to Stack Pointer
    Txs,
    /// Transfer Y to Accumulator
    Tya,
    /// Non-maskable interrupt
    Nmi,
    /// Interrupt request
    Irq,
    /// Double operation (2x NOP) <Unofficial>
    Dop,
    /// AND byte with accumulator <Unofficial>
    Aac,
    /// AND byte with accumulator, then shift accumulator right one bit <Unofficial>
    Asr,
    /// No such operation
    Invalid,
}

#[rustfmt::skip]
pub fn into_op(code: u8) -> Option<(OpKind, AddressingMode, AccessMode)> {
    Some(match code {
        0x69 => (OpKind::Adc, AddressingMode::Immediate, AccessMode::Read),
        0x65 => (OpKind::Adc, AddressingMode::ZeroPage, AccessMode::Read),
        0x75 => (OpKind::Adc, AddressingMode::ZeroPageX, AccessMode::Read),
        0x6D => (OpKind::Adc, AddressingMode::Absolute, AccessMode::Read),
        0x7D => (OpKind::Adc, AddressingMode::AbsoluteX, AccessMode::Read),
        0x79 => (OpKind::Adc, AddressingMode::AbsoluteY, AccessMode::Read),
        0x61 => (OpKind::Adc, AddressingMode::IndirectX, AccessMode::Read),
        0x71 => (OpKind::Adc, AddressingMode::IndirectY, AccessMode::Read),
        0x29 => (OpKind::And, AddressingMode::Immediate, AccessMode::Read),
        0x25 => (OpKind::And, AddressingMode::ZeroPage, AccessMode::Read),
        0x35 => (OpKind::And, AddressingMode::ZeroPageX, AccessMode::Read),
        0x2D => (OpKind::And, AddressingMode::Absolute, AccessMode::Read),
        0x3D => (OpKind::And, AddressingMode::AbsoluteX, AccessMode::Read),
        0x39 => (OpKind::And, AddressingMode::AbsoluteY, AccessMode::Read),
        0x21 => (OpKind::And, AddressingMode::IndirectX, AccessMode::Read),
        0x31 => (OpKind::And, AddressingMode::IndirectY, AccessMode::Read),
        0x0A => (OpKind::Asl, AddressingMode::Accumulator, AccessMode::ReadModifyWrite),
        0x06 => (OpKind::Asl, AddressingMode::ZeroPage, AccessMode::ReadModifyWrite),
        0x16 => (OpKind::Asl, AddressingMode::ZeroPageX, AccessMode::ReadModifyWrite),
        0x0E => (OpKind::Asl, AddressingMode::Absolute, AccessMode::ReadModifyWrite),
        0x1E => (OpKind::Asl, AddressingMode::AbsoluteX, AccessMode::ReadModifyWrite),
        0x24 => (OpKind::Bit, AddressingMode::ZeroPage, AccessMode::Read),
        0x2C => (OpKind::Bit, AddressingMode::Absolute, AccessMode::Read),
        0x4C => (OpKind::Jmp, AddressingMode::Absolute, AccessMode::Read),
        0x6C => (OpKind::Jmp, AddressingMode::Indirect, AccessMode::Read),
        0xA9 => (OpKind::Lda, AddressingMode::Immediate, AccessMode::Read),
        0xA5 => (OpKind::Lda, AddressingMode::ZeroPage, AccessMode::Read),
        0xB5 => (OpKind::Lda, AddressingMode::ZeroPageX, AccessMode::Read),
        0xAD => (OpKind::Lda, AddressingMode::Absolute, AccessMode::Read),
        0xBD => (OpKind::Lda, AddressingMode::AbsoluteX, AccessMode::Read),
        0xB9 => (OpKind::Lda, AddressingMode::AbsoluteY, AccessMode::Read),
        0xA1 => (OpKind::Lda, AddressingMode::IndirectX, AccessMode::Read),
        0xB1 => (OpKind::Lda, AddressingMode::IndirectY, AccessMode::Read),
        0xA2 => (OpKind::Ldx, AddressingMode::Immediate, AccessMode::Read),
        0xA6 => (OpKind::Ldx, AddressingMode::ZeroPage, AccessMode::Read),
        0xB6 => (OpKind::Ldx, AddressingMode::ZeroPageY, AccessMode::Read),
        0xAE => (OpKind::Ldx, AddressingMode::Absolute, AccessMode::Read),
        0xBE => (OpKind::Ldx, AddressingMode::AbsoluteY, AccessMode::Read),
        0xA0 => (OpKind::Ldy, AddressingMode::Immediate, AccessMode::Read),
        0xA4 => (OpKind::Ldy, AddressingMode::ZeroPage, AccessMode::Read),
        0xB4 => (OpKind::Ldy, AddressingMode::ZeroPageX, AccessMode::Read),
        0xAC => (OpKind::Ldy, AddressingMode::Absolute, AccessMode::Read),
        0xBC => (OpKind::Ldy, AddressingMode::AbsoluteX, AccessMode::Read),
        0x85 => (OpKind::Sta, AddressingMode::ZeroPage, AccessMode::Write),
        0x95 => (OpKind::Sta, AddressingMode::ZeroPageX, AccessMode::Write),
        0x8D => (OpKind::Sta, AddressingMode::Absolute, AccessMode::Write),
        0x9D => (OpKind::Sta, AddressingMode::AbsoluteX, AccessMode::Write),
        0x99 => (OpKind::Sta, AddressingMode::AbsoluteY, AccessMode::Write),
        0x81 => (OpKind::Sta, AddressingMode::IndirectX, AccessMode::Write),
        0x91 => (OpKind::Sta, AddressingMode::IndirectY, AccessMode::Write),
        0xC9 => (OpKind::Cmp, AddressingMode::Immediate, AccessMode::Read),
        0xC5 => (OpKind::Cmp, AddressingMode::ZeroPage, AccessMode::Read),
        0xD5 => (OpKind::Cmp, AddressingMode::ZeroPageX, AccessMode::Read),
        0xCD => (OpKind::Cmp, AddressingMode::Absolute, AccessMode::Read),
        0xDD => (OpKind::Cmp, AddressingMode::AbsoluteX, AccessMode::Read),
        0xD9 => (OpKind::Cmp, AddressingMode::AbsoluteY, AccessMode::Read),
        0xC1 => (OpKind::Cmp, AddressingMode::IndirectX, AccessMode::Read),
        0xD1 => (OpKind::Cmp, AddressingMode::IndirectY, AccessMode::Read),
        0x09 => (OpKind::Ora, AddressingMode::Immediate, AccessMode::Read),
        0x05 => (OpKind::Ora, AddressingMode::ZeroPage, AccessMode::Read),
        0x15 => (OpKind::Ora, AddressingMode::ZeroPageX, AccessMode::Read),
        0x0D => (OpKind::Ora, AddressingMode::Absolute, AccessMode::Read),
        0x1D => (OpKind::Ora, AddressingMode::AbsoluteX, AccessMode::Read),
        0x19 => (OpKind::Ora, AddressingMode::AbsoluteY, AccessMode::Read),
        0x01 => (OpKind::Ora, AddressingMode::IndirectX, AccessMode::Read),
        0x11 => (OpKind::Ora, AddressingMode::IndirectY, AccessMode::Read),
        0x49 => (OpKind::Eor, AddressingMode::Immediate, AccessMode::Read),
        0x45 => (OpKind::Eor, AddressingMode::ZeroPage, AccessMode::Read),
        0x55 => (OpKind::Eor, AddressingMode::ZeroPageX, AccessMode::Read),
        0x4D => (OpKind::Eor, AddressingMode::Absolute, AccessMode::Read),
        0x5D => (OpKind::Eor, AddressingMode::AbsoluteX, AccessMode::Read),
        0x59 => (OpKind::Eor, AddressingMode::AbsoluteY, AccessMode::Read),
        0x41 => (OpKind::Eor, AddressingMode::IndirectX, AccessMode::Read),
        0x51 => (OpKind::Eor, AddressingMode::IndirectY, AccessMode::Read),
        0x4A => (OpKind::Lsr, AddressingMode::Accumulator, AccessMode::ReadModifyWrite),
        0x4E => (OpKind::Lsr, AddressingMode::Absolute, AccessMode::ReadModifyWrite),
        0x5E => (OpKind::Lsr, AddressingMode::AbsoluteX, AccessMode::ReadModifyWrite),
        0x46 => (OpKind::Lsr, AddressingMode::ZeroPage, AccessMode::ReadModifyWrite),
        0x56 => (OpKind::Lsr, AddressingMode::ZeroPageX, AccessMode::ReadModifyWrite),
        0xE6 => (OpKind::Inc, AddressingMode::ZeroPage, AccessMode::ReadModifyWrite),
        0xF6 => (OpKind::Inc, AddressingMode::ZeroPageX, AccessMode::ReadModifyWrite),
        0xEE => (OpKind::Inc, AddressingMode::Absolute, AccessMode::ReadModifyWrite),
        0xFE => (OpKind::Inc, AddressingMode::AbsoluteX, AccessMode::ReadModifyWrite),
        0xC6 => (OpKind::Dec, AddressingMode::ZeroPage, AccessMode::ReadModifyWrite),
        0xD6 => (OpKind::Dec, AddressingMode::ZeroPageX, AccessMode::ReadModifyWrite),
        0xCE => (OpKind::Dec, AddressingMode::Absolute, AccessMode::ReadModifyWrite),
        0xDE => (OpKind::Dec, AddressingMode::AbsoluteX, AccessMode::ReadModifyWrite),
        0xE0 => (OpKind::Cpx, AddressingMode::Immediate, AccessMode::Read),
        0xE4 => (OpKind::Cpx, AddressingMode::ZeroPage, AccessMode::Read),
        0xEC => (OpKind::Cpx, AddressingMode::Absolute, AccessMode::Read),
        0xC0 => (OpKind::Cpy, AddressingMode::Immediate, AccessMode::Read),
        0xC4 => (OpKind::Cpy, AddressingMode::ZeroPage, AccessMode::Read),
        0xCC => (OpKind::Cpy, AddressingMode::Absolute, AccessMode::Read),
        0x86 => (OpKind::Stx, AddressingMode::ZeroPage, AccessMode::Write),
        0x96 => (OpKind::Stx, AddressingMode::ZeroPageY, AccessMode::Write),
        0x8E => (OpKind::Stx, AddressingMode::Absolute, AccessMode::Write),
        0x84 => (OpKind::Sty, AddressingMode::ZeroPage, AccessMode::Write),
        0x94 => (OpKind::Sty, AddressingMode::ZeroPageX, AccessMode::Write),
        0x8C => (OpKind::Sty, AddressingMode::Absolute, AccessMode::Write),
        0x20 => (OpKind::Jsr, AddressingMode::Absolute, AccessMode::Read),
        0xE9 | 0xEB => (OpKind::Sbc, AddressingMode::Immediate, AccessMode::Read),
        0xED => (OpKind::Sbc, AddressingMode::Absolute, AccessMode::Read),
        0xFD => (OpKind::Sbc, AddressingMode::AbsoluteX, AccessMode::Read),
        0xF9 => (OpKind::Sbc, AddressingMode::AbsoluteY, AccessMode::Read),
        0xE5 => (OpKind::Sbc, AddressingMode::ZeroPage, AccessMode::Read),
        0xF5 => (OpKind::Sbc, AddressingMode::ZeroPageX, AccessMode::Read),
        0xE1 => (OpKind::Sbc, AddressingMode::IndirectX, AccessMode::Read),
        0xF1 => (OpKind::Sbc, AddressingMode::IndirectY, AccessMode::Read),
        0x60 => (OpKind::Rts, AddressingMode::Implied, AccessMode::Read),
        0x00 => (OpKind::Brk, AddressingMode::Implied, AccessMode::Read),
        0xAA => (OpKind::Tax, AddressingMode::Implied, AccessMode::Read),
        0x8A => (OpKind::Txa, AddressingMode::Implied, AccessMode::Read),
        0xCA => (OpKind::Dex, AddressingMode::Implied, AccessMode::Read),
        0xE8 => (OpKind::Inx, AddressingMode::Implied, AccessMode::Read),
        0xA8 => (OpKind::Tay, AddressingMode::Implied, AccessMode::Read),
        0x98 => (OpKind::Tya, AddressingMode::Implied, AccessMode::Read),
        0x88 => (OpKind::Dey, AddressingMode::Implied, AccessMode::Read),
        0xC8 => (OpKind::Iny, AddressingMode::Implied, AccessMode::Read),
        0x9A => (OpKind::Txs, AddressingMode::Implied, AccessMode::Read),
        0xBA => (OpKind::Tsx, AddressingMode::Implied, AccessMode::Read),
        0x48 => (OpKind::Pha, AddressingMode::Implied, AccessMode::Read),
        0x68 => (OpKind::Pla, AddressingMode::Implied, AccessMode::Read),
        0x08 => (OpKind::Php, AddressingMode::Implied, AccessMode::Read),
        0x28 => (OpKind::Plp, AddressingMode::Implied, AccessMode::Read),
        0x10 => (OpKind::Bpl, AddressingMode::Relative, AccessMode::Read),
        0x30 => (OpKind::Bmi, AddressingMode::Relative, AccessMode::Read),
        0x50 => (OpKind::Bvc, AddressingMode::Relative, AccessMode::Read),
        0x70 => (OpKind::Bvs, AddressingMode::Relative, AccessMode::Read),
        0x90 => (OpKind::Bcc, AddressingMode::Relative, AccessMode::Read),
        0xB0 => (OpKind::Bcs, AddressingMode::Relative, AccessMode::Read),
        0xD0 => (OpKind::Bne, AddressingMode::Relative, AccessMode::Read),
        0xF0 => (OpKind::Beq, AddressingMode::Relative, AccessMode::Read),
        0x18 => (OpKind::Clc, AddressingMode::Implied, AccessMode::Read),
        0x38 => (OpKind::Sec, AddressingMode::Implied, AccessMode::Read),
        0x58 => (OpKind::Cli, AddressingMode::Implied, AccessMode::Read),
        0x78 => (OpKind::Sei, AddressingMode::Implied, AccessMode::Read),
        0xB8 => (OpKind::Clv, AddressingMode::Implied, AccessMode::Read),
        0xD8 => (OpKind::Cld, AddressingMode::Implied, AccessMode::Read),
        0xF8 => (OpKind::Sed, AddressingMode::Implied, AccessMode::Read),
        0x40 => (OpKind::Rti, AddressingMode::Implied, AccessMode::Read),
        0x2A => (OpKind::Rol, AddressingMode::Accumulator, AccessMode::ReadModifyWrite),
        0x26 => (OpKind::Rol, AddressingMode::ZeroPage, AccessMode::ReadModifyWrite),
        0x36 => (OpKind::Rol, AddressingMode::ZeroPageX, AccessMode::ReadModifyWrite),
        0x2E => (OpKind::Rol, AddressingMode::Absolute, AccessMode::ReadModifyWrite),
        0x3E => (OpKind::Rol, AddressingMode::AbsoluteX, AccessMode::ReadModifyWrite),
        0x6A => (OpKind::Ror, AddressingMode::Accumulator, AccessMode::ReadModifyWrite),
        0x6E => (OpKind::Ror, AddressingMode::Absolute, AccessMode::ReadModifyWrite),
        0x7E => (OpKind::Ror, AddressingMode::AbsoluteX, AccessMode::ReadModifyWrite),
        0x66 => (OpKind::Ror, AddressingMode::ZeroPage, AccessMode::ReadModifyWrite),
        0x76 => (OpKind::Ror, AddressingMode::ZeroPageX, AccessMode::ReadModifyWrite),
        0xEA | 0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA
            => (OpKind::Nop, AddressingMode::Implied, AccessMode::Read),

        0x04 | 0x44 | 0x64=> (OpKind::Dop, AddressingMode::ZeroPage, AccessMode::Read),
        0x14 | 0x34 | 0x54 | 0x74 | 0xD4 | 0xF4  => (OpKind::Dop, AddressingMode::ZeroPageX, AccessMode::Read),
        0x80 |0x82 | 0x89| 0xC2 | 0xE2 => (OpKind::Dop, AddressingMode::Immediate, AccessMode::Read),
        
        0x0B | 0x2B => (OpKind::Aac, AddressingMode::Immediate, AccessMode::Read),
        0x4B => (OpKind::Asr, AddressingMode::Immediate, AccessMode::Read),
        _ => return None,
    })
}

pub fn to_asm(op_kind: OpKind, addressing_mode: AddressingMode, val: u16) -> String {
    if op_kind == OpKind::Invalid {
        return "???".to_string();
    }

    // TODO improve (labels, effective addr, etc.)
    match addressing_mode {
        AddressingMode::Implied => format!("{:?}", op_kind),
        AddressingMode::Accumulator => format!("{:?} A", op_kind),
        AddressingMode::Immediate => format!("{:?} #${:02X}", op_kind, val),
        AddressingMode::Relative => format!("{:?} ${:04X}", op_kind, val),
        AddressingMode::Absolute => format!("{:?} ${:04X}", op_kind, val),
        AddressingMode::AbsoluteX => format!("{:?} ${:04X},X", op_kind, val),
        AddressingMode::AbsoluteY => format!("{:?} ${:04X},Y", op_kind, val),
        AddressingMode::ZeroPage => format!("{:?} ${:02X}", op_kind, val),
        AddressingMode::ZeroPageX => format!("{:?} ${:02X},X", op_kind, val),
        AddressingMode::ZeroPageY => format!("{:?} ${:02X},Y", op_kind, val),
        AddressingMode::Indirect => format!("{:?} (${:04X})", op_kind, val),
        AddressingMode::IndirectX => format!("{:?} (${:02X},X)", op_kind, val),
        AddressingMode::IndirectY => format!("{:?} (${:02X}),Y", op_kind, val),
    }
    .to_uppercase()
}

pub fn op_size(addressing_mode: AddressingMode) -> u8 {
    match addressing_mode {
        AddressingMode::Implied => 1,
        AddressingMode::Accumulator => 1,
        AddressingMode::Immediate => 2,
        AddressingMode::Relative => 2,
        AddressingMode::Absolute => 3,
        AddressingMode::AbsoluteX => 3,
        AddressingMode::AbsoluteY => 3,
        AddressingMode::ZeroPage => 2,
        AddressingMode::ZeroPageX => 2,
        AddressingMode::ZeroPageY => 2,
        AddressingMode::Indirect => 3,
        AddressingMode::IndirectX => 2,
        AddressingMode::IndirectY => 2,
    }
}

#[derive(Copy, Clone)]
pub struct CpuOpEntry {
    pub addr: u16,
    pub opcode: u8,
    pub size: u8,
    pub kind: OpKind,
    pub addr_mode: AddressingMode,
    pub operands: [u8; 2],
}
