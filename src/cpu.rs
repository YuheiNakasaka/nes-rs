use crate::opcodes::OPCODES_MAP;

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum AddressingMode {
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPage_X,
    ZeroPage_Y,
    Absolute,
    Absolute_X,
    Absolute_Y,
    Indirect,
    Indirect_X,
    Indirect_Y,
    NoneAddressing,
}

pub struct CPU {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub stack_pointer: u8,
    pub status: u8,
    pub program_counter: u16,
    memory: [u8; 0xFFFF],
}

impl CPU {
    pub fn new() -> Self {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            stack_pointer: 0,
            status: 0,
            program_counter: 0,
            memory: [0; 0xFFFF],
        }
    }

    fn mem_read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }

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
        let addr = self.get_operand_address(mode);
        let mem_value = self.mem_read(addr);
        let reg_a = self.register_a;
        let current_carry = self.status & 0b0000_0001;
        self.register_a = reg_a + mem_value + current_carry;
        self.update_zero_and_negative_flags(self.register_a);

        // NOTE: 怪しい...
        // carry flag
        if (reg_a as i32) + (mem_value as i32) + (current_carry as i32) > 0xFF {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }

        // overflow flag
        if (reg_a ^ mem_value) & 0x80 == 0 && (reg_a ^ self.register_a) & 0x80 != 0 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
        }
    }

    fn and(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_a &= value;
        self.update_zero_and_negative_flags(self.register_a);
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

    fn asl_m(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);
        if value >> 7 == 1 {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }
        value = value << 1;
        self.mem_write(addr, value);
        self.update_zero_and_negative_flags(value);
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
        let addr = self.get_operand_address(mode);
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

    fn bvc(&mut self) {}

    fn bvs(&mut self) {}

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
        let addr = self.get_operand_address(mode);
        let mem_value = self.mem_read(addr);

        if reg_value >= mem_value {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }

        self.update_zero_and_negative_flags(reg_value.wrapping_sub(mem_value));
    }

    fn dec(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
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
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_a ^= value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn inc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        let result = value.wrapping_add(1);
        self.mem_write(addr, result);
        self.update_zero_and_negative_flags(result);
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
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn ldx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_x = value;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn ldy(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_y = value;
        self.update_zero_and_negative_flags(self.register_y);
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

    fn lsr_m(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);
        if value & 1 == 1 {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }
        value = value >> 1;
        self.mem_write(addr, value);
        self.update_zero_and_negative_flags(value);
    }

    fn ora(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_a |= value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn pha(&mut self) {
        self.push_stack(self.register_a);
    }

    fn php(&mut self) {
        // https://www.nesdev.org/wiki/Status_flags
        self.status = self.status | 0b0011_0000;
        self.push_stack(self.status);
    }

    fn pla(&mut self) {
        self.register_a = self.stack_pointer;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn plp(&mut self) {
        self.status = self.stack_pointer;
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

    fn rol_m(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
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

    fn ror_m(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
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
        let addr = self.get_operand_address(mode);
        let mem_value = self.mem_read(addr);
        let reg_a = self.register_a;
        let current_carry = self.status & 0b0000_0001;
        self.register_a = reg_a - mem_value - (1 - current_carry);
        self.update_zero_and_negative_flags(self.register_a);

        // NOTE: 怪しい...
        // carry flag
        if (reg_a as i32) - (mem_value as i32) - ((1 - current_carry) as i32) >= 0 {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }

        // overflow flag
        if (reg_a ^ mem_value) & 0x80 != 0 && (reg_a ^ self.register_a) & 0x80 != 0 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
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
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_a);
    }

    fn stx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_x);
    }

    fn sty(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
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

    fn get_operand_address(&mut self, mode: &AddressingMode) -> u16 {
        match mode {
            AddressingMode::Immediate => self.program_counter,
            AddressingMode::ZeroPage => self.mem_read(self.program_counter) as u16,
            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_x) as u16;
                addr
            }
            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_y) as u16;
                addr
            }
            AddressingMode::Absolute => self.mem_read_u16(self.program_counter),
            AddressingMode::Absolute_X => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_x as u16);
                addr
            }
            AddressingMode::Absolute_Y => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_y as u16);
                addr
            }
            AddressingMode::Indirect_X => {
                let base = self.mem_read(self.program_counter) as u16;
                let ptr = (base as u8).wrapping_add(self.register_x);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                (hi as u16) << 8 | (lo as u16)
            }
            AddressingMode::Indirect_Y => {
                let base = self.mem_read(self.program_counter) as u16;
                let lo = self.mem_read(base as u16);
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.register_y as u16);
                deref
            }
            AddressingMode::Accumulator => 0 as u16, // to avoid warning
            AddressingMode::Indirect => 0 as u16,    // to avoid warning
            AddressingMode::NoneAddressing => panic!("mode: {:?} is not supported", mode),
        }
    }

    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.status = 0;
        self.program_counter = self.mem_read_u16(0xFFFC);
    }

    pub fn load(&mut self, program: Vec<u8>) {
        self.memory[0x8000..(0x8000 + program.len())].copy_from_slice(&program[..]);
        self.mem_write_u16(0xFFFC, 0x8000);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run()
    }

    pub fn run(&mut self) {
        loop {
            let code = self.mem_read(self.program_counter);
            self.program_counter += 1;
            let program_counter_state = self.program_counter;
            let opcode = OPCODES_MAP
                .get(&code)
                .expect(&format!("OpCode {:X} is not recognized", code));
            match code {
                /* ADC */
                0x69 | 0x65 | 0x75 | 0x6D | 0x7D | 0x79 | 0x61 | 0x71 => self.adc(&opcode.mode),
                0x29 | 0x25 | 0x35 | 0x2D | 0x3D | 0x39 | 0x21 | 0x31 => self.and(&opcode.mode),
                0x0a => self.asl_a(),
                0x06 | 0x16 | 0x0e | 0x1e => self.asl_m(&opcode.mode),
                0x90 => self.bcc(),
                0xB0 => self.bcs(),
                0xF0 => self.beq(),
                0x24 | 0x2C => self.bit(&opcode.mode),
                0x30 => self.bmi(),
                0xD0 => self.bne(),
                0x10 => self.bpl(),
                0x00 => self.brk(),
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
                0xE6 | 0xF6 | 0xEE | 0xFE => self.inc(&opcode.mode),
                0xE8 => self.inx(),
                0xc8 => self.iny(),
                0x20 => self.jsr(),
                0x4c => self.jmp_absolute(),
                0x6c => self.jmp_indirect(),
                0xA9 | 0xA5 | 0xB5 | 0xAD | 0xBD | 0xB9 | 0xA1 | 0xB1 => self.lda(&opcode.mode),
                0xA2 | 0xA6 | 0xB6 | 0xAE | 0xBE => self.ldx(&opcode.mode),
                0xA0 | 0xA4 | 0xB4 | 0xAC | 0xBC => self.ldy(&opcode.mode),
                0x4a => self.lsr_a(),
                0x46 | 0x56 | 0x4e | 0x5e => self.lsr_m(&opcode.mode),
                0xEA => {}
                0x09 | 0x05 | 0x15 | 0x0d | 0x1d | 0x19 | 0x01 | 0x11 => self.ora(&opcode.mode),
                0x48 => self.pha(),
                0x08 => self.php(),
                0x68 => self.pla(),
                0x28 => self.plp(),
                0x2a => self.rol_a(),
                0x26 | 0x36 | 0x2e | 0x3e => self.rol_m(&opcode.mode),
                0x6a => self.ror_a(),
                0x66 | 0x76 | 0x6e | 0x7e => self.ror_m(&opcode.mode),
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
                _ => todo!(),
            }

            if program_counter_state == self.program_counter {
                self.program_counter += (opcode.len - 1) as u16;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_0x18_clc_clear_carry_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0x18, 0x00]);
        assert!(cpu.status & 0b0000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_0xd8_cld_clear_decimal_mode() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xd8, 0x00]);
        assert!(cpu.status & 0b0000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_0x58_cli_clear_interrupt_disable() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0x58, 0x00]);
        assert!(cpu.status & 0b0000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_0xb8_clv_clear_overflow_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xb8, 0x00]);
        assert!(cpu.status & 0b0000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_0xa9_lda_immediate_load_data() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x05, 0x00]);
        assert_eq!(cpu.register_a, 0x05);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0);
    }

    #[test]
    fn test_0xa9_lda_zero_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x00, 0x00]);
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status & 0b0000_0010 == 0b10);
    }

    #[test]
    fn test_0xa9_lda_negative_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x80, 0x00]);
        assert_eq!(cpu.register_a, 0x80);
        assert!(cpu.status & 0b1000_0000 == 0b1000_0000);
    }

    #[test]
    fn test_0xa2_ldx_immediate_load_data() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa2, 0x05, 0x00]);
        assert_eq!(cpu.register_x, 0x05);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0);
    }

    #[test]
    fn test_0xa6_ldx_zero_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa6, 0x00, 0x00]);
        assert_eq!(cpu.register_x, 0x00);
        assert!(cpu.status & 0b0000_0010 == 0b10);
    }

    #[test]
    fn test_0xa0_ldy_immediate_load_data() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa0, 0x05, 0x00]);
        assert_eq!(cpu.register_y, 0x05);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0);
    }

    #[test]
    fn test_0xa4_ldy_zero_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa4, 0x00, 0x00]);
        assert_eq!(cpu.register_y, 0x00);
        assert!(cpu.status & 0b0000_0010 == 0b10);
    }

    #[test]
    fn test_0xaa_tax_move_a_to_x() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x83, 0xaa, 0x00]);
        assert_eq!(cpu.register_x, 0x83);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0b1000_0000);
    }

    #[test]
    fn test_0xa8_tay_move_a_to_x() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x83, 0xa8, 0x00]);
        assert_eq!(cpu.register_y, 0x83);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0b1000_0000);
    }

    #[test]
    fn test_0xba_tsx_move_s_to_x() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xba, 0x00]);
        assert_eq!(cpu.register_x, 0x00);
        assert!(cpu.status & 0b0000_0010 == 0b10);
        assert!(cpu.status & 0b1000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_0xba_txa_move_x_to_a() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0x8a, 0x00]);
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status & 0b0000_0010 == 0b10);
        assert!(cpu.status & 0b1000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_0xba_txs_move_x_to_s() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0x9a, 0x00]);
        assert_eq!(cpu.stack_pointer, 0x00);
    }

    #[test]
    fn test_0x98_tya_move_y_to_a() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0x98, 0x00]);
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status & 0b0000_0010 == 0b10);
        assert!(cpu.status & 0b1000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_0xe8_inx_increment_x() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x7F, 0xaa, 0xe8, 0x00]);
        assert_eq!(cpu.register_x, 0x80);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0b1000_0000);
    }

    #[test]
    fn test_5_ops_working_together() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 0xc1)
    }

    #[test]
    fn test_inx_overflow() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0xff, 0xaa, 0xe8, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 1)
    }

    #[test]
    fn test_0xc8_iny_increment_y() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x7F, 0xa8, 0xc8, 0x00]);
        assert_eq!(cpu.register_y, 0x80);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0b1000_0000);
    }

    #[test]
    fn test_0xca_dex_decrement_x() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x80, 0xaa, 0xca, 0x00]);
        assert_eq!(cpu.register_x, 0x7F);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_0x88_dey_decrement_y() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x80, 0xa8, 0x88, 0x00]);
        assert_eq!(cpu.register_y, 0x7F);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_0xc6_dec_decrement_memory() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xc6, 0x00]);
        assert_eq!(cpu.mem_read(0x00), 0xFF);
        assert!(cpu.status & 0b0000_0010 == 0b0000_0000);
        assert!(cpu.status & 0b1000_0000 == 0b1000_0000);
    }

    #[test]
    fn test_0xe6_inc_increment_memory() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xe6, 0x00]);
        assert_eq!(cpu.mem_read(0x00), 0x01);
        assert!(cpu.status & 0b0000_0010 == 0b0000_0000);
        assert!(cpu.status & 0b1000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_lda_from_memory() {
        let mut cpu = CPU::new();
        cpu.mem_write(0x10, 0x55);
        cpu.load_and_run(vec![0xA5, 0x10, 0x00]);
        assert_eq!(cpu.register_a, 0x55);
    }

    #[test]
    fn test_0x48_pha_push_accumulator() {
        let mut cpu = CPU::new();
        cpu.mem_write(0x10, 0x55);
        cpu.load_and_run(vec![0xA5, 0x10, 0xAA, 0x9A, 0x48, 0x00]);
        assert_eq!(cpu.mem_read(0x0155), 0x55);
    }

    #[test]
    fn test_0x38_sec_set_carry_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0x38, 0x00]);
        assert!(cpu.status & 0b0000_0001 == 0b0000_0001);
    }

    #[test]
    fn test_0x78_sei_set_interrupt_disable() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0x78, 0x00]);
        assert!(cpu.status & 0b0000_0100 == 0b0000_0100);
    }

    #[test]
    fn test_sta_to_memory() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x55, 0x85, 0x00]);
        assert_eq!(
            cpu.mem_read(cpu.mem_read(cpu.program_counter - 2) as u16),
            0x55
        );
    }

    #[test]
    fn test_stx_to_memory() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x55, 0xaa, 0x86, 0x00]);
        assert_eq!(
            cpu.mem_read(cpu.mem_read(cpu.program_counter - 2) as u16),
            0x55
        );
    }

    #[test]
    fn test_sty_to_memory() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x55, 0xa8, 0x84, 0x00]);
        assert_eq!(
            cpu.mem_read(cpu.mem_read(cpu.program_counter - 2) as u16),
            0x55
        );
    }

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
