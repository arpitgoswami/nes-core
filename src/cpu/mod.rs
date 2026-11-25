//! Ricoh 2A03 CPU: a 6502 core with the 2A03's missing decimal mode.
//!
//! Instructions are decoded through the table in [`opcodes`]. Base cycles come
//! from that table; indexed reads add a cycle on a page crossing, and taken
//! branches add one (plus one more when the branch crosses a page).

pub mod opcodes;

use serde::{Deserialize, Serialize};

use opcodes::{is_read_instruction, Mode, Op, TABLE};

pub const FLAG_CARRY: u8 = 0x01;
pub const FLAG_ZERO: u8 = 0x02;
pub const FLAG_IRQ_DISABLE: u8 = 0x04;
pub const FLAG_DECIMAL: u8 = 0x08;
pub const FLAG_BREAK: u8 = 0x10;
pub const FLAG_UNUSED: u8 = 0x20;
pub const FLAG_OVERFLOW: u8 = 0x40;
pub const FLAG_NEGATIVE: u8 = 0x80;

const NMI_VECTOR: u16 = 0xFFFA;
const RESET_VECTOR: u16 = 0xFFFC;
const IRQ_VECTOR: u16 = 0xFFFE;

pub trait CpuBus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, value: u8);

    /// Returns true when a new NMI edge is pending and acknowledges it.
    fn poll_nmi(&mut self) -> bool {
        false
    }

    /// Current level of the IRQ line.
    fn poll_irq(&self) -> bool {
        false
    }

    /// Cycles the CPU must stall (OAM/DMC DMA) after the current instruction.
    fn take_stall_cycles(&mut self) -> u32 {
        0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub status: u8,
    pub cycles: u64,
    pub jammed: bool,
    pub irq_line: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0,
            status: FLAG_UNUSED | FLAG_IRQ_DISABLE,
            cycles: 0,
            jammed: false,
            irq_line: false,
        }
    }

    pub fn reset<B: CpuBus>(&mut self, bus: &mut B) {
        self.sp = self.sp.wrapping_sub(3);
        self.status |= FLAG_IRQ_DISABLE;
        self.pc = self.read16(bus, RESET_VECTOR);
        self.cycles += 7;
        self.jammed = false;
    }

    pub fn flag(&self, flag: u8) -> bool {
        self.status & flag != 0
    }

    pub fn set_flag(&mut self, flag: u8, on: bool) {
        if on {
            self.status |= flag;
        } else {
            self.status &= !flag;
        }
    }

    fn set_nz(&mut self, value: u8) {
        self.set_flag(FLAG_ZERO, value == 0);
        self.set_flag(FLAG_NEGATIVE, value & 0x80 != 0);
    }

    fn read16<B: CpuBus>(&mut self, bus: &mut B, addr: u16) -> u16 {
        let lo = bus.read(addr);
        let hi = bus.read(addr.wrapping_add(1));
        u16::from_le_bytes([lo, hi])
    }

    fn fetch<B: CpuBus>(&mut self, bus: &mut B) -> u8 {
        let value = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        value
    }

    fn push<B: CpuBus>(&mut self, bus: &mut B, value: u8) {
        bus.write(0x0100 | u16::from(self.sp), value);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pull<B: CpuBus>(&mut self, bus: &mut B) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.read(0x0100 | u16::from(self.sp))
    }

    fn interrupt<B: CpuBus>(&mut self, bus: &mut B, vector: u16, brk: bool) {
        let pc = self.pc;
        self.push(bus, (pc >> 8) as u8);
        self.push(bus, pc as u8);
        let mut status = self.status | FLAG_UNUSED;
        if brk {
            status |= FLAG_BREAK;
        } else {
            status &= !FLAG_BREAK;
        }
        self.push(bus, status);
        self.set_flag(FLAG_IRQ_DISABLE, true);
        self.pc = self.read16(bus, vector);
    }

    pub fn step<B: CpuBus>(&mut self, bus: &mut B) -> u32 {
        if self.jammed {
            self.cycles += 2;
            return 2;
        }

        if bus.poll_nmi() {
            self.interrupt(bus, NMI_VECTOR, false);
            self.cycles += 7;
            self.irq_line = bus.poll_irq();
            return 7;
        }

        if self.irq_line && !self.flag(FLAG_IRQ_DISABLE) {
            self.interrupt(bus, IRQ_VECTOR, false);
            self.cycles += 7;
            self.irq_line = bus.poll_irq();
            return 7;
        }

        let opcode = self.fetch(bus);
        let (op, mode, base) = TABLE[usize::from(opcode)];
        let mut cycles = u32::from(base);

        match op {
            Op::Jam => {
                self.jammed = true;
            }
            Op::Brk => {
                self.pc = self.pc.wrapping_add(1);
                self.interrupt(bus, IRQ_VECTOR, true);
            }
            Op::Rti => {
                self.status = (self.pull(bus) & !FLAG_BREAK) | FLAG_UNUSED;
                let lo = self.pull(bus);
                let hi = self.pull(bus);
                self.pc = u16::from_le_bytes([lo, hi]);
            }
            Op::Rts => {
                let lo = self.pull(bus);
                let hi = self.pull(bus);
                self.pc = u16::from_le_bytes([lo, hi]).wrapping_add(1);
            }
            Op::Jmp => {
                let (addr, _) = self.address(bus, mode);
                self.pc = addr;
            }
            Op::Jsr => {
                let lo = self.fetch(bus);
                let hi = self.fetch(bus);
                let target = u16::from_le_bytes([lo, hi]);
                let ret = self.pc.wrapping_sub(1);
                self.push(bus, (ret >> 8) as u8);
                self.push(bus, ret as u8);
                self.pc = target;
            }
            Op::Pha => {
                let a = self.a;
                self.push(bus, a);
            }
            Op::Php => {
                let p = self.status | FLAG_BREAK | FLAG_UNUSED;
                self.push(bus, p);
            }
            Op::Pla => {
                self.a = self.pull(bus);
                self.set_nz(self.a);
            }
            Op::Plp => {
                self.status = (self.pull(bus) & !FLAG_BREAK) | FLAG_UNUSED;
            }
            Op::Tax => {
                self.x = self.a;
                self.set_nz(self.x);
            }
            Op::Tay => {
                self.y = self.a;
                self.set_nz(self.y);
            }
            Op::Txa => {
                self.a = self.x;
                self.set_nz(self.a);
            }
            Op::Tya => {
                self.a = self.y;
                self.set_nz(self.a);
            }
            Op::Tsx => {
                self.x = self.sp;
                self.set_nz(self.x);
            }
            Op::Txs => {
                self.sp = self.x;
            }
            Op::Inx => {
                self.x = self.x.wrapping_add(1);
                self.set_nz(self.x);
            }
            Op::Iny => {
                self.y = self.y.wrapping_add(1);
                self.set_nz(self.y);
            }
            Op::Dex => {
                self.x = self.x.wrapping_sub(1);
                self.set_nz(self.x);
            }
            Op::Dey => {
                self.y = self.y.wrapping_sub(1);
                self.set_nz(self.y);
            }
            Op::Clc => self.set_flag(FLAG_CARRY, false),
            Op::Sec => self.set_flag(FLAG_CARRY, true),
            Op::Cli => self.set_flag(FLAG_IRQ_DISABLE, false),
            Op::Sei => self.set_flag(FLAG_IRQ_DISABLE, true),
            Op::Clv => self.set_flag(FLAG_OVERFLOW, false),
            Op::Cld => self.set_flag(FLAG_DECIMAL, false),
            Op::Sed => self.set_flag(FLAG_DECIMAL, true),
            Op::Nop => {
                if mode != Mode::Imp && mode != Mode::Acc {
                    let (addr, crossed) = self.address(bus, mode);
                    if !matches!(
                        mode,
                        Mode::Imm | Mode::Zp | Mode::ZpX | Mode::ZpY | Mode::Abs
                    ) {
                        let _ = bus.read(addr);
                    }
                    if crossed {
                        cycles += 1;
                    }
                }
            }
            Op::Bcc | Op::Bcs | Op::Beq | Op::Bmi | Op::Bne | Op::Bpl | Op::Bvc | Op::Bvs => {
                let (target, _) = self.address(bus, mode);
                let take = match op {
                    Op::Bcc => !self.flag(FLAG_CARRY),
                    Op::Bcs => self.flag(FLAG_CARRY),
                    Op::Beq => self.flag(FLAG_ZERO),
                    Op::Bne => !self.flag(FLAG_ZERO),
                    Op::Bmi => self.flag(FLAG_NEGATIVE),
                    Op::Bpl => !self.flag(FLAG_NEGATIVE),
                    Op::Bvs => self.flag(FLAG_OVERFLOW),
                    Op::Bvc => !self.flag(FLAG_OVERFLOW),
                    _ => false,
                };
                if take {
                    cycles += 1;
                    if target & 0xFF00 != self.pc & 0xFF00 {
                        cycles += 1;
                    }
                    self.pc = target;
                }
            }
            Op::Lda | Op::Ldx | Op::Ldy | Op::Lax => {
                let (addr, crossed) = self.address(bus, mode);
                let value = bus.read(addr);
                match op {
                    Op::Lda => self.a = value,
                    Op::Ldx => self.x = value,
                    Op::Ldy => self.y = value,
                    _ => {
                        self.a = value;
                        self.x = value;
                    }
                }
                self.set_nz(value);
                if crossed && is_read_instruction(op) {
                    cycles += 1;
                }
            }
            Op::Sta | Op::Stx | Op::Sty | Op::Sax => {
                let (addr, _) = self.address(bus, mode);
                let value = match op {
                    Op::Sta => self.a,
                    Op::Stx => self.x,
                    Op::Sty => self.y,
                    _ => self.a & self.x,
                };
                bus.write(addr, value);
            }
            Op::Ora | Op::And | Op::Eor => {
                let (addr, crossed) = self.address(bus, mode);
                let value = bus.read(addr);
                match op {
                    Op::Ora => self.a |= value,
                    Op::And => self.a &= value,
                    _ => self.a ^= value,
                }
                self.set_nz(self.a);
                if crossed {
                    cycles += 1;
                }
            }
            Op::Adc => {
                let (addr, crossed) = self.address(bus, mode);
                let value = bus.read(addr);
                self.adc(value);
                if crossed {
                    cycles += 1;
                }
            }
            Op::Sbc => {
                let (addr, crossed) = self.address(bus, mode);
                let value = bus.read(addr);
                self.sbc(value);
                if crossed {
                    cycles += 1;
                }
            }
            Op::Cmp | Op::Cpx | Op::Cpy => {
                let (addr, crossed) = self.address(bus, mode);
                let value = bus.read(addr);
                let reg = match op {
                    Op::Cmp => self.a,
                    Op::Cpx => self.x,
                    _ => self.y,
                };
                self.compare(reg, value);
                if crossed {
                    cycles += 1;
                }
            }
            Op::Bit => {
                let (addr, _) = self.address(bus, mode);
                let value = bus.read(addr);
                self.set_flag(FLAG_ZERO, self.a & value == 0);
                self.set_flag(FLAG_NEGATIVE, value & 0x80 != 0);
                self.set_flag(FLAG_OVERFLOW, value & 0x40 != 0);
            }
            Op::Asl
            | Op::Lsr
            | Op::Rol
            | Op::Ror
            | Op::Inc
            | Op::Dec
            | Op::Slo
            | Op::Rla
            | Op::Sre
            | Op::Rra
            | Op::Dcp
            | Op::Isc => {
                if mode == Mode::Acc {
                    let value = self.a;
                    let result = self.read_modify_write(op, value);
                    self.a = result;
                    self.set_nz(result);
                } else {
                    let (addr, _) = self.address(bus, mode);
                    let value = bus.read(addr);
                    let result = self.read_modify_write(op, value);
                    bus.write(addr, result);
                    match op {
                        Op::Inc | Op::Dec | Op::Dcp | Op::Isc => self.set_nz(result),
                        _ => {}
                    }
                }
            }
            Op::Anc => {
                let value = self.fetch(bus);
                self.a &= value;
                self.set_nz(self.a);
                self.set_flag(FLAG_CARRY, self.a & 0x80 != 0);
            }
            Op::Alr => {
                let value = self.fetch(bus);
                self.a &= value;
                self.set_flag(FLAG_CARRY, self.a & 1 != 0);
                self.a >>= 1;
                self.set_nz(self.a);
            }
            Op::Arr => {
                let value = self.fetch(bus);
                self.a &= value;
                let carry_in = u8::from(self.flag(FLAG_CARRY));
                self.a = (self.a >> 1) | (carry_in << 7);
                self.set_nz(self.a);
                self.set_flag(FLAG_CARRY, self.a & 0x40 != 0);
                self.set_flag(FLAG_OVERFLOW, ((self.a >> 6) ^ (self.a >> 5)) & 1 != 0);
            }
            Op::Sbx => {
                let value = self.fetch(bus);
                let base = self.a & self.x;
                self.x = base.wrapping_sub(value);
                self.set_flag(FLAG_CARRY, base >= value);
                self.set_nz(self.x);
            }
        }

        let stall = bus.take_stall_cycles();
        cycles += stall;
        self.cycles += u64::from(cycles);

        // Sample the IRQ level for the next instruction boundary.
        self.irq_line = bus.poll_irq();
        cycles
    }

    fn read_modify_write(&mut self, op: Op, value: u8) -> u8 {
        match op {
            Op::Asl => {
                self.set_flag(FLAG_CARRY, value & 0x80 != 0);
                value << 1
            }
            Op::Lsr => {
                self.set_flag(FLAG_CARRY, value & 1 != 0);
                value >> 1
            }
            Op::Rol => {
                let carry = u8::from(self.flag(FLAG_CARRY));
                self.set_flag(FLAG_CARRY, value & 0x80 != 0);
                (value << 1) | carry
            }
            Op::Ror => {
                let carry = u8::from(self.flag(FLAG_CARRY));
                self.set_flag(FLAG_CARRY, value & 1 != 0);
                (value >> 1) | (carry << 7)
            }
            Op::Inc => value.wrapping_add(1),
            Op::Dec => value.wrapping_sub(1),
            Op::Slo => {
                self.set_flag(FLAG_CARRY, value & 0x80 != 0);
                let result = value << 1;
                self.a |= result;
                self.set_nz(self.a);
                result
            }
            Op::Rla => {
                let carry = u8::from(self.flag(FLAG_CARRY));
                self.set_flag(FLAG_CARRY, value & 0x80 != 0);
                let result = (value << 1) | carry;
                self.a &= result;
                self.set_nz(self.a);
                result
            }
            Op::Sre => {
                self.set_flag(FLAG_CARRY, value & 1 != 0);
                let result = value >> 1;
                self.a ^= result;
                self.set_nz(self.a);
                result
            }
            Op::Rra => {
                let carry = u8::from(self.flag(FLAG_CARRY));
                self.set_flag(FLAG_CARRY, value & 1 != 0);
                let result = (value >> 1) | (carry << 7);
                self.adc(result);
                result
            }
            Op::Dcp => {
                let result = value.wrapping_sub(1);
                self.compare(self.a, result);
                result
            }
            Op::Isc => {
                let result = value.wrapping_add(1);
                self.sbc(result);
                result
            }
            _ => value,
        }
    }

    fn adc(&mut self, value: u8) {
        let carry = u16::from(self.flag(FLAG_CARRY));
        let sum = u16::from(self.a) + u16::from(value) + carry;
        let result = sum as u8;
        self.set_flag(FLAG_CARRY, sum > 0xFF);
        self.set_flag(
            FLAG_OVERFLOW,
            (!(self.a ^ value) & (self.a ^ result) & 0x80) != 0,
        );
        self.a = result;
        self.set_nz(result);
    }

    fn sbc(&mut self, value: u8) {
        self.adc(!value);
    }

    fn compare(&mut self, register: u8, value: u8) {
        let result = register.wrapping_sub(value);
        self.set_flag(FLAG_CARRY, register >= value);
        self.set_nz(result);
    }

    fn address<B: CpuBus>(&mut self, bus: &mut B, mode: Mode) -> (u16, bool) {
        match mode {
            Mode::Imp | Mode::Acc => (0, false),
            Mode::Imm => {
                let addr = self.pc;
                self.pc = self.pc.wrapping_add(1);
                (addr, false)
            }
            Mode::Zp => (u16::from(self.fetch(bus)), false),
            Mode::ZpX => {
                let base = self.fetch(bus);
                (u16::from(base.wrapping_add(self.x)), false)
            }
            Mode::ZpY => {
                let base = self.fetch(bus);
                (u16::from(base.wrapping_add(self.y)), false)
            }
            Mode::Abs => {
                let lo = self.fetch(bus);
                let hi = self.fetch(bus);
                (u16::from_le_bytes([lo, hi]), false)
            }
            Mode::AbsX | Mode::AbsY => {
                let lo = self.fetch(bus);
                let hi = self.fetch(bus);
                let base = u16::from_le_bytes([lo, hi]);
                let index = if mode == Mode::AbsX { self.x } else { self.y };
                let addr = base.wrapping_add(u16::from(index));
                (addr, addr & 0xFF00 != base & 0xFF00)
            }
            Mode::Ind => {
                let lo = self.fetch(bus);
                let hi = self.fetch(bus);
                let ptr = u16::from_le_bytes([lo, hi]);
                // The 6502's indirect JMP page-wrap bug is reproduced here.
                let lo_addr = ptr;
                let hi_addr = (ptr & 0xFF00) | u16::from((ptr as u8).wrapping_add(1));
                let target_lo = bus.read(lo_addr);
                let target_hi = bus.read(hi_addr);
                (u16::from_le_bytes([target_lo, target_hi]), false)
            }
            Mode::IndX => {
                let base = self.fetch(bus).wrapping_add(self.x);
                let lo = bus.read(u16::from(base));
                let hi = bus.read(u16::from(base.wrapping_add(1)));
                (u16::from_le_bytes([lo, hi]), false)
            }
            Mode::IndY => {
                let base = self.fetch(bus);
                let lo = bus.read(u16::from(base));
                let hi = bus.read(u16::from(base.wrapping_add(1)));
                let ptr = u16::from_le_bytes([lo, hi]);
                let addr = ptr.wrapping_add(u16::from(self.y));
                (addr, addr & 0xFF00 != ptr & 0xFF00)
            }
            Mode::Rel => {
                let offset = self.fetch(bus) as i8;
                let target = self.pc.wrapping_add_signed(i16::from(offset));
                (target, false)
            }
        }
    }
}
