use crate::{cartridge::Rom, cpu::Mem, joypad::Joypad, ppu::NesPPU};

const RAM: u16 = 0x0000;
const RAM_MIRRORS_END: u16 = 0x1FFF;
const PPU_REGISTERS_MIRRORS_END: u16 = 0x3FFF;

pub struct Bus<'call> {
    cpu_wram: [u8; 2048], // 11bit
    prg_rom: Vec<u8>,
    ppu: NesPPU,
    cycles: usize,
    gameloop_callback: Box<dyn FnMut(&NesPPU, &mut Joypad) + 'call>,
    joypad1: Joypad,
}

impl<'a> Bus<'a> {
    pub fn new<'call, F>(rom: Rom, gameloop_callback: F) -> Bus<'call>
    where
        F: FnMut(&NesPPU, &mut Joypad) + 'call,
    {
        let ppu = NesPPU::new(rom.chr_rom, rom.screen_mirroring);
        Bus {
            cpu_wram: [0; 2048],
            prg_rom: rom.prg_rom,
            ppu: ppu,
            cycles: 0,
            gameloop_callback: Box::from(gameloop_callback),
            joypad1: Joypad::new(),
        }
    }

    pub fn tick(&mut self, cycles: u8) {
        self.cycles += cycles as usize;
        let new_frame = self.ppu.tick(cycles * 3);
        if new_frame {
            (self.gameloop_callback)(&self.ppu, &mut self.joypad1);
        }
    }

    pub fn poll_nmi_status(&mut self) -> Option<u8> {
        self.ppu.poll_nmi_interrupt()
    }

    fn read_prg_rom(&self, mut addr: u16) -> u8 {
        addr -= 0x8000;
        if self.prg_rom.len() == 0x4000 && addr >= 0x4000 {
            addr = addr % 0x4000;
        }
        self.prg_rom[addr as usize]
    }
}

impl Mem for Bus<'_> {
    fn mem_read(&mut self, addr: u16) -> u8 {
        match addr {
            RAM..=RAM_MIRRORS_END => {
                let mirror_down_addr = addr & 0b0000_0111_1111_1111;
                self.cpu_wram[mirror_down_addr as usize]
            }
            0x2000 | 0x2001 | 0x2003 | 0x2005 | 0x2006 | 0x4014 => {
                panic!("Attempt to read from write-only PPU address {:x}", addr);
            }
            0x2002 => self.ppu.read_status(),
            0x2004 => self.ppu.read_oam_data(),
            0x2007 => self.ppu.read_data(),
            0x2008..=PPU_REGISTERS_MIRRORS_END => {
                let mirror_down_addr = addr & 0b00100000_00000111;
                self.mem_read(mirror_down_addr)
            }
            0x4000..=0x4015 => 0,
            0x4016 => self.joypad1.read(),
            0x4017 => 0,
            0x8000..=0xFFFF => self.read_prg_rom(addr),
            _ => {
                // Ignoring mem access to other addresses
                0
            }
        }
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        match addr {
            RAM..=RAM_MIRRORS_END => {
                let mirror_down_addr = addr & 0b111_1111_1111;
                self.cpu_wram[mirror_down_addr as usize] = data;
            }
            0x2000 => self.ppu.write_to_ctrl(data),
            0x2001 => self.ppu.write_to_mask(data),
            0x2002 => panic!("Attempt to write to read-only PPU address {:x}", addr),
            0x2003 => self.ppu.write_to_oam_addr(data),
            0x2004 => self.ppu.write_to_oam_data(data),
            0x2005 => self.ppu.write_to_scroll(data),
            0x2006 => self.ppu.write_to_ppu_addr(data),
            0x2007 => self.ppu.write_to_data(data),
            0x2008..=PPU_REGISTERS_MIRRORS_END => {
                let mirror_down_addr = addr & 0b00100000_00000111;
                self.mem_write(mirror_down_addr, data);
            }
            0x4000..=0x4013 | 0x4015 => {}
            0x4016 => self.joypad1.write(data),
            0x4017 => {}
            0x4014 => {
                let mut buffer: [u8; 256] = [0; 256];
                let hi: u16 = (data as u16) << 8;
                for i in 0..256u16 {
                    buffer[i as usize] = self.mem_read(hi + i);
                }
                self.ppu.write_oam_dma(&buffer);
            }
            0x8000..=0xFFFF => {
                panic!("Attempted to write to Cartridge ROM space");
            }
            _ => {
                // Ignoring mem access to other addresses
            }
        }
    }
}
