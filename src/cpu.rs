use crate::interrupts::*;
use crate::{bus::Bus, opcodes::OPCODES_MAP};

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPage_X,
    ZeroPage_Y,
    Absolute,
    Absolute_X,
    Absolute_Y,
    Indirect_X,
    Indirect_Y,
    NoneAddressing,
    // Accumulator,
    // Indirect,
    // Relative,
}

pub trait Mem {
    fn mem_read(&mut self, addr: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&mut self, pos: u16) -> u16 {
        let lo = self.mem_read(pos) as u16;
        let hi = self.mem_read(pos + 1) as u16;
        (hi << 8) | (lo as u16)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.mem_write(pos, lo);
        self.mem_write(pos + 1, hi);
    }
}

impl Mem for CPU<'_> {
    fn mem_read(&mut self, addr: u16) -> u8 {
        self.bus.mem_read(addr)
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.bus.mem_write(addr, data)
    }

    fn mem_read_u16(&mut self, pos: u16) -> u16 {
        self.bus.mem_read_u16(pos)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        self.bus.mem_write_u16(pos, data)
    }
}

pub struct CPU<'a> {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub stack_pointer: u8,
    pub status: u8,
    pub program_counter: u16,
    pub bus: Bus<'a>,
}

impl<'a> CPU<'a> {
    pub fn new<'b>(bus: Bus<'b>) -> CPU<'b> {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            stack_pointer: 0xfd,
            status: 0b0010_0100,
            program_counter: 0,
            bus: bus,
        }
    }

    fn pop_stack(&mut self) -> u8 {
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        self.mem_read(0x0100 as u16 + self.stack_pointer as u16)
    }

    fn push_stack(&mut self, data: u8) {
        self.mem_write(0x0100 as u16 + self.stack_pointer as u16, data);
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    }

    fn pop_stack_u16(&mut self) -> u16 {
        let lo = self.pop_stack() as u16;
        let hi = self.pop_stack() as u16;
        hi << 8 | lo
    }

    fn push_stack_u16(&mut self, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.push_stack(hi);
        self.push_stack(lo);
    }

    fn adc(&mut self, mode: &AddressingMode) {
        let (addr, page_crossed) = self.get_operand_address(mode);
        let mem_value = self.mem_read(addr);

        let a = self.register_a.clone();
        let c = self.status & 0b0000_0001;
        let sum = a as u16 + mem_value as u16 + c as u16;

        // carry flag
        if sum > 0xFF {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }

        // overflow flag
        let result = sum as u8;
        if (mem_value ^ result) & (result ^ self.register_a) & 0x80 != 0 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
        }

        // set accumulator
        self.register_a = result;
        self.update_zero_and_negative_flags(self.register_a);

        if page_crossed {
            self.bus.tick(1);
        }
    }

    fn and(&mut self, mode: &AddressingMode) {
        let (addr, page_crossed) = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_a &= value;
        self.update_zero_and_negative_flags(self.register_a);

        if page_crossed {
            self.bus.tick(1);
        }
    }

    fn asl_a(&mut self) {
        let mut value = self.register_a;
        if value >> 7 == 1 {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }
        value = value << 1;
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn asl_m(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);
        if value >> 7 == 1 {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }
        value = value << 1;
        self.mem_write(addr, value);
        self.update_zero_and_negative_flags(value);
        value
    }

    fn bcc(&mut self) {
        self.branch(self.status & 0b0000_0001 == 0)
    }

    fn bcs(&mut self) {
        self.branch(self.status & 0b0000_0001 != 0)
    }

    fn beq(&mut self) {
        self.branch(self.status & 0b0000_0010 != 0)
    }

    fn bit(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        let mem_value = self.mem_read(addr);

        // V
        if (mem_value & 0b0100_0000) >> 6 == 1 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
        }

        // N
        if mem_value >> 7 == 1 {
            self.status = self.status | 0b1000_0000;
        } else {
            self.status = self.status & 0b0111_1111;
        }

        // Z = A & M
        if self.register_a & mem_value == 0 {
            self.status = self.status | 0b0000_0010;
        } else {
            self.status = self.status & 0b1111_1101;
        }
    }

    fn bmi(&mut self) {
        self.branch(self.status & 0b1000_0000 != 0)
    }

    fn bne(&mut self) {
        self.branch(self.status & 0b0000_0010 == 0)
    }

    fn bpl(&mut self) {
        self.branch(self.status & 0b1000_0000 == 0)
    }

    fn brk(&mut self) {
        return;
    }

    fn bvc(&mut self) {
        self.branch(self.status & 0b0100_0000 == 0)
    }

    fn bvs(&mut self) {
        self.branch(self.status & 0b0100_0000 != 0)
    }

    fn clc(&mut self) {
        self.status = self.status & 0b1111_1110
    }

    fn cld(&mut self) {
        self.status = self.status & 0b1111_0111
    }

    fn cli(&mut self) {
        self.status = self.status & 0b1111_1011
    }

    fn clv(&mut self) {
        self.status = self.status & 0b1011_1111
    }

    fn cmp(&mut self, mode: &AddressingMode, reg_value: u8) {
        let (addr, page_crossed) = self.get_operand_address(mode);
        let mem_value = self.mem_read(addr);

        if reg_value >= mem_value {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }

        self.update_zero_and_negative_flags(reg_value.wrapping_sub(mem_value));

        if page_crossed {
            self.bus.tick(1);
        }
    }

    fn dec(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        let result = value.wrapping_sub(1);
        self.mem_write(addr, result);
        self.update_zero_and_negative_flags(result);
    }

    fn dex(&mut self) {
        if self.register_x == 0x00 {
            self.register_x = 0xff;
        } else {
            self.register_x -= 1;
        }
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn dey(&mut self) {
        if self.register_y == 0x00 {
            self.register_y = 0xff;
        } else {
            self.register_y -= 1;
        }
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn eor(&mut self, mode: &AddressingMode) {
        let (addr, page_crossed) = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_a ^= value;
        self.update_zero_and_negative_flags(self.register_a);

        if page_crossed {
            self.bus.tick(1);
        }
    }

    fn inc(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        let result = value.wrapping_add(1);
        self.mem_write(addr, result);
        self.update_zero_and_negative_flags(result);
        result
    }

    fn inx(&mut self) {
        if self.register_x == 0xff {
            self.register_x = 0;
        } else {
            self.register_x += 1;
        }
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn iny(&mut self) {
        if self.register_y == 0xff {
            self.register_y = 0;
        } else {
            self.register_y += 1;
        }
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn jmp_absolute(&mut self) {
        self.program_counter = self.mem_read_u16(self.program_counter);
    }

    // NOTE: An original 6502 has does not correctly fetch the target address if the indirect vector falls on a page boundary (e.g. $xxFF where xx is any value from $00 to $FF). In this case fetches the LSB from $xxFF as expected but takes the MSB from $xx00. This is fixed in some later chips like the 65SC02 so for compatibility always ensure the indirect vector is not at the end of the page.
    // https://www.nesdev.org/obelisk-6502-guide/reference.html#JMP
    fn jmp_indirect(&mut self) {
        let addr = self.mem_read_u16(self.program_counter);
        let indirect_addr = if addr & 0x00ff == 0x00ff {
            let lo = self.mem_read(addr) as u16;
            let hi = self.mem_read(addr & 0xff00) as u16;
            hi << 8 | lo
        } else {
            self.mem_read_u16(addr)
        };

        self.program_counter = indirect_addr;
    }

    fn jsr(&mut self) {
        // 仕様としては2を足せばいいだけだが命令の読み込みで既に+1をしてる分-1して帳尻合わせしてる
        self.push_stack_u16(self.program_counter + 2 - 1);
        self.program_counter = self.mem_read_u16(self.program_counter);
    }

    fn lda(&mut self, mode: &AddressingMode) {
        let (addr, page_crossed) = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);

        if page_crossed {
            self.bus.tick(1);
        }
    }

    fn ldx(&mut self, mode: &AddressingMode) {
        let (addr, page_crossed) = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_x = value;
        self.update_zero_and_negative_flags(self.register_x);

        if page_crossed {
            self.bus.tick(1);
        }
    }

    fn ldy(&mut self, mode: &AddressingMode) {
        let (addr, page_crossed) = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_y = value;
        self.update_zero_and_negative_flags(self.register_y);

        if page_crossed {
            self.bus.tick(1);
        }
    }

    fn lsr_a(&mut self) {
        let mut value = self.register_a;
        if value & 1 == 1 {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }
        value = value >> 1;
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn lsr_m(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);
        if value & 1 == 1 {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }
        value = value >> 1;
        self.mem_write(addr, value);
        self.update_zero_and_negative_flags(value);
        value
    }

    fn ora(&mut self, mode: &AddressingMode) {
        let (addr, page_crossed) = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_a |= value;
        self.update_zero_and_negative_flags(self.register_a);

        if page_crossed {
            self.bus.tick(1);
        }
    }

    fn pha(&mut self) {
        self.push_stack(self.register_a);
    }

    fn php(&mut self) {
        // https://www.nesdev.org/wiki/Status_flags
        let flag = self.status | 0b0011_0000;
        self.push_stack(flag);
    }

    fn pla(&mut self) {
        self.register_a = self.pop_stack();
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn plp(&mut self) {
        self.status = self.pop_stack();
        self.status = self.status & 0b1110_1111;
        self.status = self.status | 0b0010_0000;
    }

    fn rol_a(&mut self) {
        let mut value = self.register_a;
        let current_carry = self.status & 0b0000_0001;
        if value >> 7 == 1 {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }
        value = value << 1;
        if current_carry == 1 {
            value = value | 1;
        }
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn rol_m(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);
        let current_carry = self.status & 0b0000_0001;
        if value >> 7 == 1 {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }
        value = value << 1;
        if current_carry == 1 {
            value = value | 1;
        }
        self.mem_write(addr, value);
        self.update_zero_and_negative_flags(value);
        value
    }

    fn ror_a(&mut self) {
        let mut value = self.register_a;
        let current_carry = self.status & 0b0000_0001;
        if value & 1 == 1 {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }
        value = value >> 1;
        if current_carry == 1 {
            value = value | 0b1000_0000;
        }
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn ror_m(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);
        let current_carry = self.status & 0b0000_0001;
        if value & 1 == 1 {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }
        value = value >> 1;
        if current_carry == 1 {
            value = value | 0b1000_0000;
        }
        self.mem_write(addr, value);
        self.update_zero_and_negative_flags(value);
        value
    }

    fn rti(&mut self) {
        self.status = self.pop_stack();
        self.status = self.status & 0b1110_1111;
        self.status = self.status | 0b0010_0000;
        self.program_counter = self.pop_stack_u16();
    }

    fn rts(&mut self) {
        self.program_counter = self.pop_stack_u16() + 1;
    }

    fn sbc(&mut self, mode: &AddressingMode) {
        let (addr, page_crossed) = self.get_operand_address(mode);
        let mem_value = self.mem_read(addr);

        let a = self.register_a.clone();
        let b = (mem_value as i8).wrapping_neg().wrapping_sub(1) as u8;
        let c = self.status & 0b0000_0001;

        // A - B - (1 - C) = A + (-B) - 1 + C = A + (-B - 1) + C
        let sum = a as u16
            // (-B - 1)
            + b as u16
            + c as u16;

        // carry flag
        if sum > 0xFF {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }

        // overflow flag
        let result = sum as u8;
        if (b ^ result) & (result ^ self.register_a) & 0x80 != 0 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
        }

        // set accumulator
        self.register_a = result;
        self.update_zero_and_negative_flags(self.register_a);

        if page_crossed {
            self.bus.tick(1);
        }
    }

    fn sec(&mut self) {
        self.status = self.status | 0b0000_0001
    }

    fn sed(&mut self) {
        self.status = self.status | 0b0000_1000
    }

    fn sei(&mut self) {
        self.status = self.status | 0b0000_0100
    }

    fn sta(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        self.mem_write(addr, self.register_a);
    }

    fn stx(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        self.mem_write(addr, self.register_x);
    }

    fn sty(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        self.mem_write(addr, self.register_y);
    }

    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn tay(&mut self) {
        self.register_y = self.register_a;
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn tsx(&mut self) {
        self.register_x = self.stack_pointer;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn txa(&mut self) {
        self.register_a = self.register_x;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn txs(&mut self) {
        self.stack_pointer = self.register_x;
    }

    fn tya(&mut self) {
        self.register_a = self.register_y;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn branch(&mut self, condition: bool) {
        if condition {
            // -128~127
            let jump = self.mem_read(self.program_counter) as i8;
            let jump_addr = self
                .program_counter
                .wrapping_add(1)
                .wrapping_add(jump as u16);
            self.program_counter = jump_addr;
        }
    }

    fn unofficial_isb(&mut self, mode: &AddressingMode) {
        let data = self.inc(mode);

        let a = self.register_a.clone();
        let b = (data as i8).wrapping_neg().wrapping_sub(1) as u8;
        let c = self.status & 0b0000_0001;

        // A - B - (1 - C) = A + (-B) - 1 + C = A + (-B - 1) + C
        let sum = a as u16
            // (-B - 1)
            + b as u16
            + c as u16;

        // carry flag
        if sum > 0xFF {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }

        // overflow flag
        let result = sum as u8;
        if (b ^ result) & (result ^ self.register_a) & 0x80 != 0 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
        }

        // set accumulator
        self.register_a = result;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn unofficial_slo(&mut self, mode: &AddressingMode) {
        let data = self.asl_m(mode);
        self.register_a = data | self.register_a;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn unofficial_rla(&mut self, mode: &AddressingMode) {
        let data = self.rol_m(mode);
        self.register_a = data & self.register_a;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn unofficial_sre(&mut self, mode: &AddressingMode) {
        let data = self.lsr_m(mode);
        self.register_a = data ^ self.register_a;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn unofficial_rra(&mut self, mode: &AddressingMode) {
        let data = self.ror_m(mode);

        // TODO: 共通化するためあとでリファクタリング
        let a = self.register_a.clone();
        let c = self.status & 0b0000_0001;
        let sum = a as u16 + data as u16 + c as u16;

        // carry flag
        if sum > 0xFF {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }

        // overflow flag
        let result = sum as u8;
        if (data ^ result) & (result ^ self.register_a) & 0x80 != 0 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
        }

        // set accumulator
        self.register_a = result;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn update_zero_and_negative_flags(&mut self, result: u8) {
        if result == 0 {
            self.status = self.status | 0b0000_0010;
        } else {
            self.status = self.status & 0b1111_1101;
        }

        if result & 0b1000_0000 != 0 {
            self.status = self.status | 0b1000_0000;
        } else {
            self.status = self.status & 0b0111_1111;
        }
    }

    fn get_operand_address(&mut self, mode: &AddressingMode) -> (u16, bool) {
        match mode {
            AddressingMode::Immediate => (self.program_counter, false),
            AddressingMode::ZeroPage => (self.mem_read(self.program_counter) as u16, false),
            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_x) as u16;
                (addr, false)
            }
            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_y) as u16;
                (addr, false)
            }
            AddressingMode::Absolute => (self.mem_read_u16(self.program_counter), false),
            AddressingMode::Absolute_X => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_x as u16);
                (addr, self.page_cross(base, addr))
            }
            AddressingMode::Absolute_Y => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_y as u16);
                (addr, self.page_cross(base, addr))
            }
            AddressingMode::Indirect_X => {
                let base = self.mem_read(self.program_counter) as u16;
                let ptr = (base as u8).wrapping_add(self.register_x);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                ((hi as u16) << 8 | (lo as u16), false)
            }
            AddressingMode::Indirect_Y => {
                let base = self.mem_read(self.program_counter) as u16;
                let lo = self.mem_read(base as u16);
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.register_y as u16);
                (deref, self.page_cross(deref, deref_base))
            }
            AddressingMode::NoneAddressing => panic!("mode: {:?} is not supported", mode),
        }
    }

    pub fn get_absolute_address(&mut self, mode: &AddressingMode, addr: u16) -> u16 {
        match mode {
            AddressingMode::ZeroPage => self.mem_read(addr) as u16,

            AddressingMode::Absolute => self.mem_read_u16(addr),

            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(addr);
                let addr = pos.wrapping_add(self.register_x) as u16;
                addr
            }
            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(addr);
                let addr = pos.wrapping_add(self.register_y) as u16;
                addr
            }

            AddressingMode::Absolute_X => {
                let base = self.mem_read_u16(addr);
                let addr = base.wrapping_add(self.register_x as u16);
                addr
            }
            AddressingMode::Absolute_Y => {
                let base = self.mem_read_u16(addr);
                let addr = base.wrapping_add(self.register_y as u16);
                addr
            }

            AddressingMode::Indirect_X => {
                let base = self.mem_read(addr);

                let ptr: u8 = (base as u8).wrapping_add(self.register_x);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                (hi as u16) << 8 | (lo as u16)
            }
            AddressingMode::Indirect_Y => {
                let base = self.mem_read(addr);

                let lo = self.mem_read(base as u16);
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.register_y as u16);
                deref
            }
            _ => {
                panic!("mode {:?} is not supported", mode);
            }
        }
    }

    fn interrupt(&mut self, interrupt: interrupts::Interrupt) {
        self.push_stack_u16(self.program_counter);
        let mut flag = self.status.clone();
        if interrupt.b_flag_mask & 0b010000 == 1 {
            flag = flag | 0b0001_0000;
        } else {
            flag = flag & 0b1110_1111;
        }
        if interrupt.b_flag_mask & 0b100000 == 1 {
            flag = flag | 0b0010_0000;
        } else {
            flag = flag & 0b1101_1111;
        }

        self.push_stack(flag);
        self.status = self.status | 0b0000_0100;

        self.bus.tick(interrupt.cpu_cycles);
        self.program_counter = self.mem_read_u16(interrupt.vector_addr);
    }

    fn page_cross(&self, addr1: u16, addr2: u16) -> bool {
        addr1 & 0xFF00 != addr2 & 0xFF00
    }

    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.stack_pointer = 0xFD;
        self.status = 0b0010_0100;
        self.program_counter = self.mem_read_u16(0xFFFC);
    }

    pub fn load(&mut self, program: Vec<u8>) {
        for i in 0..(program.len() as u16) {
            self.mem_write(0x0600 + i, program[i as usize]);
        }
        // self.mem_write_u16(0xFFFC, 0x8600);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run()
    }

    pub fn run(&mut self) {
        self.run_with_callback(|_| {});
    }

    pub fn run_with_callback<F>(&mut self, mut callback: F)
    where
        F: FnMut(&mut CPU),
    {
        let ref opcodes = OPCODES_MAP;

        loop {
            if let Some(_nmi) = self.bus.poll_nmi_status() {
                self.interrupt(interrupts::NMI);
            }

            callback(self);

            let code = self.mem_read(self.program_counter);
            self.program_counter += 1;
            let program_counter_state = self.program_counter;
            let opcode = opcodes.get(&code).unwrap();

            match code {
                0x69 | 0x65 | 0x75 | 0x6D | 0x7D | 0x79 | 0x61 | 0x71 => self.adc(&opcode.mode),
                0x29 | 0x25 | 0x35 | 0x2D | 0x3D | 0x39 | 0x21 | 0x31 => self.and(&opcode.mode),
                0x0a => self.asl_a(),
                0x06 | 0x16 | 0x0e | 0x1e => {
                    self.asl_m(&opcode.mode);
                }
                0x90 => self.bcc(),
                0xB0 => self.bcs(),
                0xF0 => self.beq(),
                0x24 | 0x2C => self.bit(&opcode.mode),
                0x30 => self.bmi(),
                0xD0 => self.bne(),
                0x10 => self.bpl(),
                0x00 => return self.brk(),
                0x50 => self.bvc(),
                0x70 => self.bvs(),
                0x18 => self.clc(),
                0xD8 => self.cld(),
                0x58 => self.cli(),
                0xB8 => self.clv(),
                0xc9 | 0xc5 | 0xd5 | 0xcd | 0xdd | 0xd9 | 0xc1 | 0xd1 => {
                    self.cmp(&opcode.mode, self.register_a)
                }
                0xE0 | 0xE4 | 0xEC => self.cmp(&opcode.mode, self.register_x),
                0xC0 | 0xC4 | 0xCc => self.cmp(&opcode.mode, self.register_y),
                0xC6 | 0xD6 | 0xCE | 0xDE => self.dec(&opcode.mode),
                0xCA => self.dex(),
                0x88 => self.dey(),
                0x49 | 0x45 | 0x55 | 0x4d | 0x5d | 0x59 | 0x41 | 0x51 => self.eor(&opcode.mode),
                0xE6 | 0xF6 | 0xEE | 0xFE => {
                    self.inc(&opcode.mode);
                }
                0xE8 => self.inx(),
                0xc8 => self.iny(),
                0x20 => self.jsr(),
                0x4c => self.jmp_absolute(),
                0x6c => self.jmp_indirect(),
                0xA9 | 0xA5 | 0xB5 | 0xAD | 0xBD | 0xB9 | 0xA1 | 0xB1 => self.lda(&opcode.mode),
                0xA2 | 0xA6 | 0xB6 | 0xAE | 0xBE => self.ldx(&opcode.mode),
                0xA0 | 0xA4 | 0xB4 | 0xAC | 0xBC => self.ldy(&opcode.mode),
                0x4a => self.lsr_a(),
                0x46 | 0x56 | 0x4e | 0x5e => {
                    self.lsr_m(&opcode.mode);
                }
                0xEA => {}
                0x09 | 0x05 | 0x15 | 0x0d | 0x1d | 0x19 | 0x01 | 0x11 => self.ora(&opcode.mode),
                0x48 => self.pha(),
                0x08 => self.php(),
                0x68 => self.pla(),
                0x28 => self.plp(),
                0x2a => self.rol_a(),
                0x26 | 0x36 | 0x2e | 0x3e => {
                    self.rol_m(&opcode.mode);
                }
                0x6a => self.ror_a(),
                0x66 | 0x76 | 0x6e | 0x7e => {
                    self.ror_m(&opcode.mode);
                }
                0x40 => self.rti(),
                0x60 => self.rts(),
                0xe9 | 0xe5 | 0xf5 | 0xed | 0xfd | 0xf9 | 0xe1 | 0xf1 => self.sbc(&opcode.mode),
                0x38 => self.sec(),
                0xf8 => self.sed(),
                0x78 => self.sei(),
                0x85 | 0x95 | 0x8D | 0x9D | 0x99 | 0x81 | 0x91 => self.sta(&opcode.mode),
                0x86 | 0x96 | 0x8E => self.stx(&opcode.mode),
                0x84 | 0x94 | 0x8C => self.sty(&opcode.mode),
                0xAA => self.tax(),
                0xA8 => self.tay(),
                0xBA => self.tsx(),
                0x8A => self.txa(),
                0x9A => self.txs(),
                0x98 => self.tya(),
                // unofficial opcodes
                // https://www.nesdev.org/wiki/Programming_with_unofficial_opcodes
                /* NOPs */
                // IGN a/ IGN a,X/ IGN d / IGN d,X
                0x04 | 0x44 | 0x64 | 0x14 | 0x34 | 0x54 | 0x74 | 0xd4 | 0xf4 | 0x0c | 0x1c
                | 0x3c | 0x5c | 0x7c | 0xdc | 0xfc => {
                    let (addr, page_crossed) = self.get_operand_address(&opcode.mode);
                    self.mem_read(addr);

                    if page_crossed {
                        self.bus.tick(1);
                    }
                }
                0x02 | 0x12 | 0x22 | 0x32 | 0x42 | 0x52 | 0x62 | 0x72 | 0x92 | 0xb2 | 0xd2
                | 0xf2 => {}
                // NOP
                0x1a | 0x3a | 0x5a | 0x7a | 0xda | 0xfa => {}
                // SKB
                0x80 | 0x82 | 0x89 | 0xc2 | 0xe2 => {}
                /* LAX */
                0xa7 | 0xb7 | 0xaf | 0xbf | 0xa3 | 0xb3 => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);
                    self.register_a = data;
                    self.update_zero_and_negative_flags(self.register_a);
                    self.register_x = self.register_a;
                }
                /* SAX */
                0x87 | 0x97 | 0x8f | 0x83 => {
                    let data = self.register_a & self.register_x;
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    self.mem_write(addr, data);
                }
                /* SBC */
                0xeb => self.sbc(&opcode.mode),
                /* DCP */
                0xc7 | 0xd7 | 0xCF | 0xdF | 0xdb | 0xd3 | 0xc3 => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let mut data = self.mem_read(addr);
                    data = data.wrapping_sub(1);
                    self.mem_write(addr, data);

                    if data <= self.register_a {
                        self.status = self.status | 0x0000_0001;
                    }

                    self.update_zero_and_negative_flags(self.register_a.wrapping_sub(data));
                }
                /* ISB */
                0xe7 | 0xf7 | 0xef | 0xff | 0xfb | 0xe3 | 0xf3 => self.unofficial_isb(&opcode.mode),
                /* SLO */
                0x07 | 0x17 | 0x0F | 0x1f | 0x1b | 0x03 | 0x13 => self.unofficial_slo(&opcode.mode),
                /* RLA */
                0x27 | 0x37 | 0x2F | 0x3F | 0x3b | 0x33 | 0x23 => self.unofficial_rla(&opcode.mode),
                /* SRE */
                0x47 | 0x57 | 0x4F | 0x5f | 0x5b | 0x43 | 0x53 => self.unofficial_sre(&opcode.mode),
                /* RRA */
                0x67 | 0x77 | 0x6f | 0x7f | 0x7b | 0x63 | 0x73 => self.unofficial_rra(&opcode.mode),
                /* AXS */
                0xCB => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);
                    let x_and_a = self.register_x & self.register_a;
                    let result = x_and_a.wrapping_sub(data);

                    if data <= x_and_a {
                        self.status = self.status | 0b0000_0001;
                    }
                    self.update_zero_and_negative_flags(result);

                    self.register_x = result;
                }
                /* ARR */
                0x6B => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);
                    self.register_a = data & self.register_a;
                    self.update_zero_and_negative_flags(self.register_a);
                    self.ror_a();

                    let result = self.register_a;
                    let bit_5 = (result >> 5) & 1;
                    let bit_6 = (result >> 6) & 1;

                    if bit_6 == 1 {
                        self.status = self.status | 0b0000_0001;
                    } else {
                        self.status = self.status & 0b1111_1110;
                    }

                    if bit_5 ^ bit_6 == 1 {
                        self.status = self.status | 0b0100_0000;
                    } else {
                        self.status = self.status & 0b1011_1111;
                    }

                    self.update_zero_and_negative_flags(result);
                }
                /* ANC */
                0x0b | 0x2b => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);
                    self.register_a = data & self.register_a;
                    self.update_zero_and_negative_flags(self.register_a);
                    if self.status == 0b1000_0000 {
                        self.status = self.status | 0b0000_0001;
                    } else {
                        self.status = self.status & 0b1111_1110;
                    }
                }
                /* ALR */
                0x4b => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);
                    self.register_a = data & self.register_a;
                    self.update_zero_and_negative_flags(self.register_a);
                    self.lsr_a();
                }
                /* LXA */
                0xab => {
                    self.lda(&opcode.mode);
                    self.tax();
                }
                /* XAA */
                0x8b => {
                    self.register_a = self.register_x;
                    self.update_zero_and_negative_flags(self.register_a);
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);
                    self.register_a = data & self.register_a;
                    self.update_zero_and_negative_flags(self.register_a);
                }
                /* LAS */
                0xbb => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let mut data = self.mem_read(addr);
                    data = data & self.stack_pointer;
                    self.register_a = data;
                    self.register_x = data;
                    self.stack_pointer = data;
                    self.update_zero_and_negative_flags(data);
                }
                /* TAS */
                0x9b => {
                    let data = self.register_a & self.register_x;
                    self.stack_pointer = data;
                    let mem_address =
                        self.mem_read_u16(self.program_counter) + self.register_y as u16;

                    let data = ((mem_address >> 8) as u8 + 1) & self.stack_pointer;
                    self.mem_write(mem_address, data)
                }
                /* AHX  Indirect Y */
                0x93 => {
                    let pos: u8 = self.mem_read(self.program_counter);
                    let mem_address = self.mem_read_u16(pos as u16) + self.register_y as u16;
                    let data = self.register_a & self.register_x & (mem_address >> 8) as u8;
                    self.mem_write(mem_address, data)
                }
                /* AHX Absolute Y*/
                0x9f => {
                    let mem_address =
                        self.mem_read_u16(self.program_counter) + self.register_y as u16;

                    let data = self.register_a & self.register_x & (mem_address >> 8) as u8;
                    self.mem_write(mem_address, data)
                }
                /* SHX */
                0x9e => {
                    let mem_address =
                        self.mem_read_u16(self.program_counter) + self.register_y as u16;

                    let data = self.register_x & ((mem_address >> 8) as u8 + 1);
                    self.mem_write(mem_address, data)
                }
                /* SHY */
                0x9c => {
                    let mem_address =
                        self.mem_read_u16(self.program_counter) + self.register_x as u16;
                    let data = self.register_y & ((mem_address >> 8) as u8 + 1);
                    self.mem_write(mem_address, data)
                }
            }

            self.bus.tick(opcode.cycles);

            if program_counter_state == self.program_counter {
                self.program_counter += (opcode.len - 1) as u16;
            }
        }
    }
}

#[cfg(test)]
mod test {
    // TODO: AND/EOR/ORA
    // TODO: ASL/LSR/ROL/ROR
    // TODO: PHP/PLA/PLP
    // TODO: RTI/RTS
    // TODO: JSR/JMP
    // TODO: SBC
    // TODO: CMP/CPX/CPY
    // TODO: BCC/BCS/BEQ/BMI/BNE/BPL/BVC/BVS/BIT
    // TODO: ADC
}
