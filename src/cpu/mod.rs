#![allow(non_snake_case)]

mod flags;
mod instruction;
use self::flags::Flags;
use self::instruction::Instruction;
use core::cmp::{Eq, PartialEq, Ord, PartialOrd};
use core::ops::{Index, IndexMut};
#[cfg(feature = "logging")]
use log::*;
use States::*;

// Base cycle count per opcode. Page-crossing penalties for indexed reads and
// the extra cycles of taken branches are added on top. JAM really halts the CPU;
// 11 matches what the SingleStepTests suite records for it.
#[rustfmt::skip]
const CYCLES: [u8; 256] = [
//  0  1   2  3  4  5  6  7  8  9  A  B  C  D  E  F
    7, 6, 11, 8, 3, 3, 5, 5, 3, 2, 2, 2, 4, 4, 6, 6, // 0
    2, 5, 11, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7, // 1
    6, 6, 11, 8, 3, 3, 5, 5, 4, 2, 2, 2, 4, 4, 6, 6, // 2
    2, 5, 11, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7, // 3
    6, 6, 11, 8, 3, 3, 5, 5, 3, 2, 2, 2, 3, 4, 6, 6, // 4
    2, 5, 11, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7, // 5
    6, 6, 11, 8, 3, 3, 5, 5, 4, 2, 2, 2, 5, 4, 6, 6, // 6
    2, 5, 11, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7, // 7
    2, 6, 2, 6, 3, 3, 3, 3, 2, 2, 2, 2, 4, 4, 4, 4,  // 8
    2, 6, 11, 6, 4, 4, 4, 4, 2, 5, 2, 5, 5, 5, 5, 5, // 9
    2, 6, 2, 6, 3, 3, 3, 3, 2, 2, 2, 2, 4, 4, 4, 4,  // A
    2, 5, 11, 5, 4, 4, 4, 4, 2, 4, 2, 4, 4, 4, 4, 4, // B
    2, 6, 2, 8, 3, 3, 5, 5, 2, 2, 2, 2, 4, 4, 6, 6,  // C
    2, 5, 11, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7, // D
    2, 6, 2, 8, 3, 3, 5, 5, 2, 2, 2, 2, 4, 4, 6, 6,  // E
    2, 5, 11, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7, // F
];

// Indexed reads that take one more cycle when the index crosses a page.
// Stores and read-modify-write instructions always take the fixed count.
fn has_page_penalty(op: u8) -> bool {
    matches!(
        op,
        0x11 | 0x31 | 0x51 | 0x71 | 0xB1 | 0xD1 | 0xF1 | 0xB3
            | 0x19 | 0x39 | 0x59 | 0x79 | 0xB9 | 0xD9 | 0xF9 | 0xBB | 0xBE | 0xBF
            | 0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC | 0xBC
            | 0x1D | 0x3D | 0x5D | 0x7D | 0xBD | 0xDD | 0xFD
    )
}

#[derive(PartialEq, Clone, Copy, Eq, Ord, PartialOrd, Debug)]
enum States {
    Fetch,
    Decode,
    Execute,
}

#[derive(Clone, Debug)]
pub struct Cpu {
    //#[cfg(feature = "logging")]
    //pub log_line: String,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: Flags,
    pub sp: u8,
    pub pc: u16,
    pub addr: Option<u16>,
    pub cycles: isize,
    pub in_nmi: bool,
    pub instruction: Instruction,
    states: States,
    current_instr: fn(&mut Cpu, &mut dyn IndexMut<u16, Output = u8>),
}
impl PartialEq for Cpu {
    fn eq(&self, other: &Cpu) -> bool {
        self.a == other.a
            && self.x == other.x
            && self.y == other.y
            && self.s == other.s
            && self.sp == other.sp
            && self.pc == other.pc
    }
}
impl Eq for Cpu {}
impl Cpu {
    pub fn new_test(pc: u16, sp: u8, a: u8, x: u8, y: u8, p: u8) -> Cpu {
        let mut status = Flags::new();
        status.set(p);
        Cpu {
            a,
            x,
            y,
            s: status,
            sp,
            pc,
            addr: None,
            cycles: 0,
            in_nmi: false,
            instruction: Instruction(0xEA),
            states: Fetch,
            current_instr: Cpu::NOP,
        }
    }
    pub fn new(init_pc: Option<u16>) -> Cpu {
        let i_pc: u16;
        if let Some(pc) = init_pc {
            i_pc = pc;
        } else {
            i_pc = 0;
        }
        Cpu {
            //#[cfg(feature = "logging")]
            //log_line: "".to_string(),
            a: 0,
            x: 0,
            y: 0,
            s: Flags::new(),
            sp: 0xFD,
            pc: i_pc,
            addr: None,
            cycles: 0,
            in_nmi: false,
            instruction: Instruction(0xEA),
            states: Fetch,
            current_instr: Cpu::NOP,
        }
    }
    pub fn load16_instrs(&self, mem: &mut dyn IndexMut<u16, Output = u8>, addr: u16) -> u16{
        let b0 = mem[addr];
        let addrp1 =addr.wrapping_add(1);
        let b1 = mem[addrp1];
        u16::from_le_bytes([b0, b1])
    }
    pub fn pop_load16(&self, mem: &mut dyn IndexMut<u16, Output = u8>) -> u16{
        let b0 = mem[self.sp.wrapping_add(1) as u16 + 0x100];
        let b1 = mem[self.sp.wrapping_add(2) as u16 + 0x100];
        u16::from_le_bytes([b0, b1])
    }
    pub fn load16(&self, mem: &mut dyn IndexMut<u16, Output = u8>, addr: u16) -> u16 {
        let addr2: u16;
        if addr == 0xFF {
            addr2 = 0x0
        } else {
            addr2 = addr + 1;
        }
        let b0 = mem[addr];
        let b1 = mem[addr2];
        u16::from_le_bytes([b0, b1])
    }
    pub fn store16(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>, addr: u16, val: u16) {
        let v = val.to_le_bytes();
        mem[addr] = v[0];
        mem[addr.wrapping_add(1)] = v[1];
    }
    pub fn StackPush(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>, val:u8){
        mem[self.sp as u16 + 0x100] = val;
        if self.sp == 0x00{
            self.sp = 0xFF;
        } else {
          self.sp -= 1;
        }

    }
    pub fn irq(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        if !self.s.get_interrupt() {
            self.interrupt(mem, 0xFFFE);
        }
    }
    pub fn nmi(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.in_nmi = true;
        self.interrupt(mem, 0xFFFA);
    }
    // hardware interrupts push P with B clear (BRK/PHP push it set)
    fn interrupt(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>, vector: u16) {
        let pc = self.pc;
        self.StackPush(mem, (pc >> 8) as u8);
        self.StackPush(mem, pc as u8);
        self.StackPush(mem, (self.s.get() | 0x20) & 0xEF);
        self.s.set_interrupt(true);
        self.pc = self.load16(mem, vector);
        self.cycles += 7;
    }
    pub fn start(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.cycles += 7;
        let reset: u16 = self.load16(mem, 0xFFFC);
        self.pc = reset;
    }
    pub fn run_instr(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let pc = self.pc;
        let val = mem[pc];
        self.pc = self.pc.wrapping_add(1);
        self.cycles += 1;
        self.instruction.set(val);
        self.current_instr = self.decode(mem);
        (self.current_instr)(self, mem);
        //self.cycles+=1;
    }
    pub fn run(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) -> isize {
        self.cycles = 0;
        match self.states {
            Fetch => {
                self.fetch(mem);
                self.states = Decode;
            }
            Decode => {
                self.current_instr = self.decode(mem);
                self.states = Execute;
            }
            Execute => {
                (self.current_instr)(self, mem);
                self.states = Fetch;
            }
        }
        self.cycles
    }
    fn fetch(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let pc = self.pc;
        let val = mem[pc];
        self.pc = self.pc.wrapping_add(1);
        self.cycles += 1;
        self.instruction.set(val);
        //self.log_line = format!("{:04X}  {:02X}                                        A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} CYC:{:}\n",pc,val,self.a,self.x,self.y,self.s.get(),self.sp, self.cycles);
        //trace!("{:04X}  {:02X}        A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} CYC:{:08X}",pc,val,self.a,self.x,self.y,self.s.get(),self.sp, self.cycles);
    }
    fn decode(
        &mut self,
        mem: &mut dyn IndexMut<u16, Output = u8>,
    ) -> fn(&mut Cpu, &mut dyn IndexMut<u16, Output = u8>) {
        self.cycles += CYCLES[self.instruction.get() as usize] as isize - 1;
        match self.instruction.get() {
            0x00 => Cpu::BRK,
            0x02 | 0x12 | 0x22 | 0x32 | 0x42 | 0x52 | 0x62 | 0x72 | 0x92 | 0xB2 | 0xD2 | 0xF2 => {
                Cpu::JAM
            }
            0x03 => {
                self.addr = Some(self.indexed_indirect_x(mem));
                Cpu::SLO
            }
            0x07 => {
                self.addr = Some(self.zero_page(mem));
                Cpu::SLO
            }
            0x0F => {
                self.addr = Some(self.absolute(mem));
                Cpu::SLO
            }
            0x08 => Cpu::PHP,
            0x0B | 0x2B => {
                self.addr = Some(self.immediate());
                Cpu::ANC
            }
            0x13 =>{
                self.addr = Some(self.indirect_indexed_y(mem));
                Cpu::SLO
            }
            0x17 =>{
                self.addr = Some(self.zero_page_r(mem));
                Cpu::SLO
            }
            0x18 => Cpu::CLC,
            0x1B =>{
                self.addr = Some(self.absolute_y(mem));
                Cpu::SLO
            }
            0x1F =>{
                self.addr = Some(self.absolute_x(mem));
                Cpu::SLO
            }
            0x20 => {
                self.addr = Some(self.absolute(mem));
                Cpu::JSR
            }
            0x23 => {
                self.addr = Some(self.indexed_indirect_x(mem));
                Cpu::RLA
            }
            0x26 => {
                self.addr = Some(self.zero_page(mem));
                Cpu::ROL
            }
            0x27 => {
                self.addr = Some(self.zero_page(mem));
                Cpu::RLA
            }
            0x28 => Cpu::PLP,
            0x2F => {
                self.addr = Some(self.absolute(mem));
                Cpu::RLA
            }
            0x33 => {
                self.addr = Some(self.indirect_indexed_y(mem));
                Cpu::RLA
            }
            0x37 => {
                self.addr = Some(self.zero_page_r(mem));
                Cpu::RLA
            }
            0x38 => Cpu::SEC,
            0x3B => {
                self.addr = Some(self.absolute_y(mem));
                Cpu::RLA
            }
            0x3F => {
                self.addr = Some(self.absolute_x(mem));
                Cpu::RLA
            }
            0x40 => Cpu::RTI,
            0x43 => {
                self.addr = Some(self.indexed_indirect_x(mem));
                Cpu::SRE
            }
            0x47 =>{
                self.addr = Some(self.zero_page(mem));
                Cpu::SRE
            }
            0x48 => Cpu::PHA,
            0x4B => {
                self.addr = Some(self.immediate());
                Cpu::ALR
            },
            0x4F =>{
                self.addr = Some(self.absolute(mem));
                Cpu::SRE
            }
            0x53 => {
                self.addr = Some(self.indirect_indexed_y(mem));
                Cpu::SRE
            }
            0x57 => {
                self.addr = Some(self.zero_page_r(mem));
                Cpu::SRE
            }
            0x58 => Cpu::CLI,
            0x5B => {
                self.addr = Some(self.absolute_y(mem));
                Cpu::SRE
            }
            0x5D => {
                self.addr = Some(self.absolute_x(mem));
                Cpu::EOR
            }
            0x5F => {
                self.addr = Some(self.absolute_x(mem));
                Cpu::SRE
            }
            0x60 => Cpu::RTS,
            0x61 => {
                self.addr = Some(self.indexed_indirect_x(mem));
                Cpu::ADC
            }
            0x63 => {
                self.addr = Some(self.indexed_indirect_x(mem));
                Cpu::RRA
            }
            0x68 => Cpu::PLA,
            0x78 => Cpu::SEI,
            0x88 => Cpu::DEY,
            0x8A => Cpu::TXA,
            0x98 => Cpu::TYA,
            0x9A => Cpu::TXS,
            0xA8 => Cpu::TAY,
            0xAA => Cpu::TAX,
            0xB8 => Cpu::CLV,
            0xBA => Cpu::TSX,
            0xC8 => Cpu::INY,
            0xCA => Cpu::DEX,
            0xD8 => Cpu::CLD,
            0xE8 => Cpu::INX,
            0xEA | 0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => Cpu::NOP,
            0x80 | 0x82 | 0x89 | 0xC2 | 0xE2 => {
                self.addr = Some(self.immediate());
                Cpu::NOP
            }
            0x04 | 0x44 | 0x64 => {
                self.addr = Some(self.zero_page(mem));
                Cpu::NOP
            }
            0x14 | 0x34 | 0x54 | 0x74 | 0xD4 | 0xF4 => {
                self.addr = Some(self.zero_page_r(mem));
                Cpu::NOP
            }
            0x0C => {
                self.addr = Some(self.absolute(mem));
                Cpu::NOP
            }
            0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => {
                self.addr = Some(self.absolute_x(mem));
                Cpu::NOP
            }
            0x67 => {
                self.addr = Some(self.zero_page(mem));
                Cpu::RRA
            }
            0x6B => {
                self.addr = Some(self.immediate());
                Cpu::ARR
            }
            0x6F => {
                self.addr = Some(self.absolute(mem));
                Cpu::RRA
            }
            0x73 => {
                self.addr = Some(self.indirect_indexed_y(mem));
                Cpu::RRA
            }
            0x77 => {
                self.addr = Some(self.zero_page_r(mem));
                Cpu::RRA
            }
            0x7B => {
                self.addr = Some(self.absolute_y(mem));
                Cpu::RRA
            }
            0x7F => {
                self.addr = Some(self.absolute_x(mem));
                Cpu::RRA
            }
            0x83 => {
                self.addr = Some(self.indexed_indirect_x(mem));
                Cpu::SAX
            }
            0x87 => {
                self.addr = Some(self.zero_page(mem));
                Cpu::SAX
            }
            0x8B => {
                self.addr = Some(self.immediate());
                Cpu::ANE
            }
            0x8F => {
                self.addr = Some(self.absolute(mem));
                Cpu::SAX
            }
            0x93 => {
                self.addr = Some(self.indirect_indexed_y(mem));
                Cpu::SHA
            }
            0x97 => {
                self.addr = Some(self.zero_page_y(mem));
                Cpu::SAX
            }
            0x9B => {
                self.addr = Some(self.absolute_y(mem));
                Cpu::TAS
            }
            0x9C => {
                self.addr = Some(self.absolute_x(mem));
                Cpu::SHY
            }
            0x9E => {
                self.addr = Some(self.absolute_y(mem));
                Cpu::SHX
            }
            0x9F => {
                self.addr = Some(self.absolute_y(mem));
                Cpu::SHA
            }
            0xA3 => {
                self.addr = Some(self.indexed_indirect_x(mem));
                Cpu::LAX
            }
            0xA7 => {
                self.addr = Some(self.zero_page(mem));
                Cpu::LAX
            }
            0xAB => {
                self.addr = Some(self.immediate());
                Cpu::LXA
            }
            0xAF => {
                self.addr = Some(self.absolute(mem));
                Cpu::LAX
            }
            0xB3 => {
                self.addr = Some(self.indirect_indexed_y(mem));
                Cpu::LAX
            }
            0xB7 => {
                self.addr = Some(self.zero_page_y(mem));
                Cpu::LAX
            }
            0xBB => {
                self.addr = Some(self.absolute_y(mem));
                Cpu::LAS
            }
            0xBF => {
                self.addr = Some(self.absolute_y(mem));
                Cpu::LAX
            }
            0xC3 => {
                self.addr = Some(self.indexed_indirect_x(mem));
                Cpu::DCP
            }
            0xC7 => {
                self.addr = Some(self.zero_page(mem));
                Cpu::DCP
            }
            0xCB => {
                self.addr = Some(self.immediate());
                Cpu::SBX
            }
            0xCF => {
                self.addr = Some(self.absolute(mem));
                Cpu::DCP
            }
            0xD3 => {
                self.addr = Some(self.indirect_indexed_y(mem));
                Cpu::DCP
            }
            0xD7 => {
                self.addr = Some(self.zero_page_r(mem));
                Cpu::DCP
            }
            0xDB => {
                self.addr = Some(self.absolute_y(mem));
                Cpu::DCP
            }
            0xDF => {
                self.addr = Some(self.absolute_x(mem));
                Cpu::DCP
            }
            0xE3 => {
                self.addr = Some(self.indexed_indirect_x(mem));
                Cpu::ISC
            }
            0xE7 => {
                self.addr = Some(self.zero_page(mem));
                Cpu::ISC
            }
            0xEB => {
                self.addr = Some(self.immediate());
                Cpu::SBC
            }
            0xEF => {
                self.addr = Some(self.absolute(mem));
                Cpu::ISC
            }
            0xF3 => {
                self.addr = Some(self.indirect_indexed_y(mem));
                Cpu::ISC
            }
            0xF7 => {
                self.addr = Some(self.zero_page_r(mem));
                Cpu::ISC
            }
            0xFB => {
                self.addr = Some(self.absolute_y(mem));
                Cpu::ISC
            }
            0xFF => {
                self.addr = Some(self.absolute_x(mem));
                Cpu::ISC
            }

            0xF8 => Cpu::SED,
            _ => match self.instruction.cc() {
                0 => {
                    if let Some(i) = self.addressing0(mem) {
                        self.addr = Some(i);
                        match self.instruction.aaa() {
                            1 => Cpu::BIT,
                            2 => Cpu::JMP,
                            3 => Cpu::JMI,
                            4 => Cpu::STY,
                            5 => Cpu::LDY,
                            6 => Cpu::CPY,
                            7 => Cpu::CPX,
                            _ => panic!(),
                        }
                    } else {
                        Cpu::relative
                    }
                }
                1 => {
                    self.addressing1(mem);
                    match self.instruction.aaa() {
                        0 => Cpu::ORA,
                        1 => Cpu::AND,
                        2 => Cpu::EOR,
                        3 => Cpu::ADC,
                        4 => Cpu::STA,
                        5 => Cpu::LDA,
                        6 => Cpu::CMP,
                        7 => Cpu::SBC,
                        _ => panic!(),
                    }
                }
                2 => {
                    self.addressing2(mem);
                    match self.instruction.aaa() {
                        0 => Cpu::ASL,
                        1 => Cpu::ROL,
                        2 => Cpu::LSR,
                        3 => Cpu::ROR,
                        4 => Cpu::STX,
                        5 => Cpu::LDX,
                        6 => Cpu::DEC,
                        7 => Cpu::INC,
                        _ => panic!(),
                    }
                }
                _ => panic!("instr:{:02x}", self.instruction.0),
            },
        }
    }
    fn addressing0(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) -> Option<u16> {
        match self.instruction.bbb() {
            0 => Some(self.immediate()),
            1 => Some(self.zero_page(mem)),
            3 => Some(self.absolute(mem)),
            4 => None,
            5 => Some(self.zero_page_r(mem)),
            7 => Some(self.absolute_x(mem)),
            _ => None,
        }
    }
    fn addressing1(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.addr = Some(match self.instruction.bbb() {
            0 => self.indexed_indirect_x(mem),
            1 => self.zero_page(mem),
            2 => self.immediate(),
            3 => self.absolute(mem),
            4 => self.indirect_indexed_y(mem),
            5 => self.zero_page_r(mem),
            6 => self.absolute_y(mem),
            7 => self.absolute_x(mem),
            _ => 0,
        })
    }
    fn addressing2(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.addr = match self.instruction.bbb() {
            0 => Some(self.immediate()),
            1 => Some(self.zero_page(mem)),
            2 => None,
            3 => Some(self.absolute(mem)),
            5 => Some(self.zero_page_r2(mem)),
            7 => Some(match self.instruction.aaa() {
                5 => self.absolute_y(mem),
                _ => self.absolute_x(mem),
            }),
            _ => None,
        }
    }
    fn indirect_indexed_y(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) -> u16 {
        let pc = self.pc;
        let id = mem[pc];
        let idix = self.load16(mem, id as u16);
        let idixy = idix.wrapping_add(self.y as u16);
        self.page_penalty(idix, idixy);
        self.pc = self.pc.wrapping_add(1);
        idixy
    }
    fn indexed_indirect_x(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) -> u16 {
        let pc = self.pc;
        let x = self.x;
        let id = mem[pc].wrapping_add(x);
        self.pc = self.pc.wrapping_add(1);
        let ixid = self.load16(mem, id as u16);
        ixid
    }
    fn immediate(&mut self) -> u16 {
        self.pc = self.pc.wrapping_add(1);
        self.pc.wrapping_sub(1)
    }
    fn zero_page(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) -> u16 {
        let pc = self.pc;
        let zp = mem[pc];
        self.pc = self.pc.wrapping_add(1);
        zp as u16
    }
    fn zero_page_r(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) -> u16 {
        let pc = self.pc;
        let zpr = mem[pc];
        self.pc = self.pc.wrapping_add(1);
        zpr.wrapping_add(self.x) as u16
    }
    fn zero_page_r2(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) -> u16 {
        let pc = self.pc;
        let zpr = mem[pc];
        self.pc = self.pc.wrapping_add(1);
        match self.instruction.aaa() {
            4 | 5 => zpr.wrapping_add(self.y) as u16,
            _ => zpr.wrapping_add(self.x) as u16,
        }
    }
    fn zero_page_y(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) -> u16 {
        let pc = self.pc;
        let zpr = mem[pc];
        self.pc = self.pc.wrapping_add(1);
        zpr.wrapping_add(self.y) as u16
    }
    fn absolute(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) -> u16 {
        let id = self.load16_instrs(mem, self.pc);
        self.pc = self.pc.wrapping_add(2);
        id
    }
    fn absolute_x(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) -> u16 {
        let pc = self.pc;
        let mut idx = self.load16_instrs(mem, pc);
        let base = idx;
        idx =  idx.wrapping_add(self.x as u16);
        self.page_penalty(base, idx);
        self.pc = self.pc.wrapping_add(2);
        idx
    }
    fn absolute_y(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) -> u16 {
        let pc = self.pc;
        let mut idy = self.load16_instrs(mem, pc);
        let base = idy;
        idy = idy.wrapping_add(self.y as u16);
        self.page_penalty(base, idy);
        self.pc = self.pc.wrapping_add(2);
        idy
    }
    fn page_penalty(&mut self, base: u16, addr: u16) {
        if (base ^ addr) & 0xFF00 != 0 && has_page_penalty(self.instruction.get()) {
            self.cycles += 1;
        }
    }
    fn relative(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        match self.instruction.xx() {
            0 => {
                if self.s.get_negative() == self.instruction.y() {
                    self.branch(mem)
                }
            }
            1 => {
                if self.s.get_overflow() == self.instruction.y() {
                    self.branch(mem)
                }
            }
            2 => {
                if self.s.get_carry() == self.instruction.y() {
                    self.branch(mem)
                }
            }
            3 => {
                if self.s.get_zero() == self.instruction.y() {
                    self.branch(mem)
                }
            }
            _ => {}
        };
        self.pc = self.pc.wrapping_add(1);
    }
    fn branch(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let pc = self.pc;
        let offset = mem[pc] as i8;
        let next = pc.wrapping_add(1);
        let target = next.wrapping_add(offset as u16);
        // relative() still adds 1 for the operand byte
        self.pc = target.wrapping_sub(1);
        self.cycles += 1;
        if (next ^ target) & 0xFF00 != 0 {
            self.cycles += 1;
        }
    }
    fn JAM(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {}
    fn ORA(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr;
        let m = mem[addr.unwrap()];
        let a = self.a | m;
        self.set_flags_z_n(a);
        self.a = a;
    }
    fn AND(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr;
        let m = mem[addr.unwrap()];
        let a = self.a & m;
        self.set_flags_z_n(a);
        self.a = a;
    }
    fn EOR(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr.unwrap();
        let m = mem[addr];
        let a = self.a ^ m;
        self.a = a;
        self.set_flags_z_n(a);
    }
    fn ADC(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let m = mem[self.addr.unwrap()];
        self.add_with_carry(m);
    }
    // binary-mode add only: the NES 2A03 has no decimal mode
    fn add_with_carry(&mut self, m: u8) {
        let tmp = m as u16 + self.a as u16 + self.s.get_carry() as u16;
        let res = tmp as u8;
        let o = (!(self.a ^ m) & (self.a ^ res) & 0x80) == 0x80;
        self.a = res;
        self.set_flags_z_n_c_o(res, o, tmp > 0xFF);
    }
    fn compare(&mut self, reg: u8, m: u8) {
        self.set_flags_z_n_c(reg.wrapping_sub(m), reg >= m);
    }
    fn STA(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr;
        let a = self.a;
        mem[addr.unwrap()] = a;
    }
    fn LDA(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr;
        let val = mem[addr.unwrap()];
        self.set_flags_z_n(val);
        self.a = val;
    }
    fn CMP(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr;
        let val = mem[addr.unwrap()];
        let (res, o) = self.a.overflowing_sub(val);
        self.set_flags_z_n_c(res, !o);
    }
    fn SBC(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let m = mem[self.addr.unwrap()];
        self.add_with_carry(!m);
    }
    //{ "name": "0e 86 ef", "initial": { "pc": 254, "s": 226, "a": 7, "x": 99, "y": 166, "p": 239, "ram": [ [254, 14], [255, 134], [256, 239], [61318, 149], [257, 103]]}, "final": { "pc": 257, "s": 226, "a": 7, "x": 99, "y": 166, "p": 109, "ram": [ [254, 14], [255, 134], [256, 239], [257, 103], [61318, 42]]}, "cycles": [ [254, 14, "read"], [255, 134, "read"], [256, 239, "read"], [61318, 149, "read"], [61318, 149, "write"], [61318, 42, "write"]] },

    fn ASL(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        if let Some(addr) = self.addr {
            //println!("addr: {:x}", addr);
            let mut m = mem[addr];
            let m2 = m;
            m = m << 1;
            mem[addr] = m;
            self.set_flags_z_n_c(m, m2 & 0x80 == 0x80);
        } else {
            let a1 = self.a << 1;
            let a2 = self.a;
            self.a = a1;
            self.set_flags_z_n_c(a1, a2 & 0x80 == 0x80);
        }
    }
    fn ROL(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        if let Some(addr) = self.addr {
            let m = mem[addr];
            let m2 = m;
            let (m, _) = m.overflowing_mul(2);
            let (m, _) = m.overflowing_add(self.s.get_carry() as u8);
            mem[addr] = m;
            self.set_flags_z_n_c(m, m2 & 0x80 == 0x80);
        } else {
            let (a1, _) = self.a.overflowing_mul(2);
            let (a1, _) = a1.overflowing_add(self.s.get_carry() as u8);
            let a2 = self.a;
            self.a = a1;
            self.set_flags_z_n_c(a1, a2 & 0x80 == 0x80);
        }
    }
    fn LSR(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        if let Some(addr) = self.addr {
            let mut m = mem[addr];
            let m2 = m;
            m = m >> 1;
            mem[addr] = m;
            self.set_flags_z_n_c(m, m2 & 1 == 1);
        } else {
            let a1 = self.a >> 1;
            let a2 = self.a;
            self.a = a1;
            self.set_flags_z_n_c(a1, a2 & 1 == 1);
        }
    }
    fn ROR(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        if let Some(addr) = self.addr {
            let mut m = mem[addr];
            let m2 = m;
            m = m / 2 + ((self.s.get_carry() as u8) << 7);
            mem[addr] = m;
            self.set_flags_z_n_c(m, m2 & 1 == 1);
        } else {
            let a1 = self.a / 2 + ((self.s.get_carry() as u8) << 7);
            let a2 = self.a;
            self.a = a1 as u8;
            self.set_flags_z_n_c(a1 as u8, a2 & 1 == 1);
        }
    }
    fn STX(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        if let Some(addr) = self.addr {
            let x = self.x;
            mem[addr] = x;
        }
    }
    fn LDX(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        if let Some(addr) = self.addr {
            let val = mem[addr];
            self.set_flags_z_n(val);
            self.x = val;
        }
    }
    fn DEC(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        if let Some(addr) = self.addr {
            let m = mem[addr].wrapping_sub(1);
            mem[addr] = m;
            self.set_flags_z_n(m);
        }
    }
    fn INC(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        if let Some(addr) = self.addr {
            let m = mem[addr].wrapping_add(1);
            mem[addr] = m;
            self.set_flags_z_n(m);
        }
    }
    fn BIT(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr;
        let m = mem[addr.unwrap()];
        let res = self.a & m;
        self.set_flags_z_n_o(res, m);
    }
    fn JMP(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.pc = self.addr.unwrap();
    }
    fn JMI(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr;
        let valL = mem[addr.unwrap()];
        let addrH = self.addr.unwrap() & 0xFF00;
        let addrL = self.addr.unwrap() as u8;
        let addr2 = addrH | addrL.wrapping_add(1) as u16;
        let valH = mem[addr2];
        self.pc = valL as u16 | (valH as u16) << 8;
    }
    fn STY(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let y = self.y;
        let addr = self.addr;
        mem[addr.unwrap()] = y;
    }
    fn LDY(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr;
        let m = mem[addr.unwrap()];
        self.y = m;
        self.set_flags_z_n(m);
    }
    fn CPY(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr;
        let val = mem[addr.unwrap()];
        let y = self.y;
        let (res, o) = y.overflowing_sub(val);
        self.set_flags_z_n_c(res, !o);
    }
    fn CPX(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr;
        let val = mem[addr.unwrap()];
        let x = self.x;
        let (res, o) = x.overflowing_sub(val);
        self.set_flags_z_n_c(res, !o);
    }
    fn BRK(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let pc = self.pc.wrapping_add(1);
        self.StackPush(mem, (pc >> 8) as u8);
        self.StackPush(mem, pc as u8);
        self.StackPush(mem, self.s.get() | 0x10);
        self.s.set_interrupt(true);
        self.pc = self.load16(mem, 0xFFFE);
    }
    fn JSR(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let pc = self.pc.wrapping_sub(1);
        self.StackPush(mem,(pc >> 8) as u8);
        self.StackPush(mem, pc as u8);
        self.pc = self.addr.unwrap();
    }
    fn RTI(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.in_nmi = false;
        self.sp = self.sp.wrapping_add(1);
        let s = mem[self.sp as u16 + 0x100];
        self.s.set((s | 0x20) & 0xEF);
        self.sp = self.sp.wrapping_add(1);
        let pc_lo = mem[self.sp as u16 + 0x100];
        self.sp = self.sp.wrapping_add(1);
        let pc_hi = mem[self.sp as u16 + 0x100];
        self.pc = u16::from_le_bytes([pc_lo, pc_hi]);
    }
    fn RTS(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.sp = self.sp.wrapping_add(1);
        let pc_lo = mem[self.sp as u16 + 0x100];
        self.sp = self.sp.wrapping_add(1);
        let pc_hi = mem[self.sp as u16 + 0x100];
        self.pc = u16::from_le_bytes([pc_lo, pc_hi]);
        self.pc = self.pc.wrapping_add(1);
    }
    fn PHP(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.StackPush(mem, self.s.get() | 0x10);
    }
    fn PLP(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.sp = self.sp.wrapping_add(1);
        let sp: u16 = self.sp as u16 + 0x100;
        let p = mem[sp];
        self.s.set((p | 0x20) & 0xEF);
    }
    fn PHA(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.StackPush(mem,self.a);
    }
    fn PLA(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.sp = self.sp.wrapping_add(1);
        let a = mem[self.sp as u16 + 0x100];
        self.a = a;
        self.set_flags_z_n(a);
    }
    fn DEY(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let y = self.y.wrapping_sub(1);
        self.y = y;
        self.set_flags_z_n(y);
    }
    fn TAY(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let a = self.a;
        self.y = a;
        self.set_flags_z_n(a);
    }
    fn INY(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let y = self.y.wrapping_add(1);
        self.y = y;
        self.set_flags_z_n(y);
    }
    fn INX(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let x = self.x.wrapping_add(1);
        self.x = x;
        self.set_flags_z_n(x);
    }
    fn CLC(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.s.set_carry(false);
    }
    fn SEC(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.s.set_carry(true);
    }
    fn CLI(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.s.set_interrupt(false);
    }
    fn SEI(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.s.set_interrupt(true);
    }
    fn TYA(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let y = self.y;
        self.a = y;
        self.set_flags_z_n(y);
    }
    fn CLV(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.s.set_overflow(false);
    }
    fn CLD(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.s.set_decimal(false);
    }
    fn SED(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.s.set_decimal(true);
    }
    fn TXA(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let x = self.x;
        self.a = x;
        self.set_flags_z_n(x);
    }
    fn TXS(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.sp = self.x;
    }
    fn TAX(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let a = self.a;
        self.x = a;
        self.set_flags_z_n(a);
    }
    fn TSX(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let s = self.sp;
        self.x = s;
        self.set_flags_z_n(s);
    }
    fn DEX(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let x = self.x.wrapping_sub(1);
        self.x = x;
        self.set_flags_z_n(x);
    }
    fn NOP(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {}
    fn SLO(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr.unwrap();
        let m2 = mem[addr];
        let m = mem[addr] << 1;
        self.a |= m;
        mem[addr] = m;
        self.set_flags_z_n_c(self.a, m2 & 0x80 == 0x80);
    }
    fn ANC(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr.unwrap();
        let m = mem[addr];
        self.a &= m;
        self.set_flags_z_n_c(self.a, self.a & 0x80 == 0x80);
    }
    fn RLA(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr.unwrap();
        let m = mem[addr];
        let m2 = m;
        let (m, _) = m.overflowing_mul(2);
        let (m, _) = m.overflowing_add(self.s.get_carry() as u8);
        mem[addr] = m;
        self.a &= m;
        self.set_flags_z_n_c(self.a, m2 & 0x80 == 0x80);
    }
    fn SRE(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>){
        let addr = self.addr.unwrap();
        let m2 = mem[addr];
        let m = mem[addr] >> 1;
        self.a ^= m;
        mem[addr] = m;
        self.set_flags_z_n_c(self.a, m2 & 0x01 == 0x01);
    }
    fn ALR(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>){
        let imm = mem[self.addr.unwrap()];
        let imm2 = imm;
        let a = self.a;
        self.a &= imm;
        self.a = self.a >> 1;
        self.set_flags_z_n_c(self.a,(a & imm) & 0x01 == 0x01);
    }
    fn RRA(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>){
        let addr = self.addr.unwrap();
        let m2 = mem[addr];
        let m = ((self.s.get_carry() as u8) << 7) | (m2 >> 1);
        mem[addr] = m;
        self.s.set_carry(m2 & 0x01 == 0x01);
        self.add_with_carry(m);
    }
    fn SAX(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        mem[self.addr.unwrap()] = self.a & self.x;
    }
    fn LAX(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let m = mem[self.addr.unwrap()];
        self.a = m;
        self.x = m;
        self.set_flags_z_n(m);
    }
    fn LAS(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let m = mem[self.addr.unwrap()] & self.sp;
        self.a = m;
        self.x = m;
        self.sp = m;
        self.set_flags_z_n(m);
    }
    fn DCP(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr.unwrap();
        let m = mem[addr].wrapping_sub(1);
        mem[addr] = m;
        self.compare(self.a, m);
    }
    fn ISC(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let addr = self.addr.unwrap();
        let m = mem[addr].wrapping_add(1);
        mem[addr] = m;
        self.add_with_carry(!m);
    }
    fn ARR(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let a = self.a & mem[self.addr.unwrap()];
        let a = ((self.s.get_carry() as u8) << 7) | (a >> 1);
        self.a = a;
        self.set_flags_z_n(a);
        self.s.set_carry(a & 0x40 == 0x40);
        self.s.set_overflow(((a >> 6) ^ (a >> 5)) & 1 == 1);
    }
    // ANE and LXA are unstable on real hardware; 0xEE is the constant the test suite uses
    fn ANE(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let a = (self.a | 0xEE) & self.x & mem[self.addr.unwrap()];
        self.a = a;
        self.set_flags_z_n(a);
    }
    fn LXA(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let a = (self.a | 0xEE) & mem[self.addr.unwrap()];
        self.a = a;
        self.x = a;
        self.set_flags_z_n(a);
    }
    fn SBX(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        let m = mem[self.addr.unwrap()];
        let ax = self.a & self.x;
        self.x = ax.wrapping_sub(m);
        self.compare(ax, m);
    }
    // SHA/SHX/SHY/TAS store reg & (base_high + 1); when indexing crosses a page
    // that value also replaces the high byte of the effective address
    fn store_and_high(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>, val: u8, index: u8) {
        let addr = self.addr.unwrap();
        let base = addr.wrapping_sub(index as u16);
        let v = val & ((base >> 8) as u8).wrapping_add(1);
        let addr = if (base ^ addr) & 0xFF00 != 0 {
            ((v as u16) << 8) | (addr & 0x00FF)
        } else {
            addr
        };
        mem[addr] = v;
    }
    fn SHA(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.store_and_high(mem, self.a & self.x, self.y);
    }
    fn SHX(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.store_and_high(mem, self.x, self.y);
    }
    fn SHY(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.store_and_high(mem, self.y, self.x);
    }
    fn TAS(&mut self, mem: &mut dyn IndexMut<u16, Output = u8>) {
        self.sp = self.a & self.x;
        self.store_and_high(mem, self.sp, self.y);
    }
    fn set_flags_z_n(&mut self, res: u8) {
        self.s.set_zero(res == 0);
        self.s.set_negative((res & 0x80) == 0x80);
    }

    fn set_flags_z_n_c(&mut self, res: u8, o: bool) {
        self.set_flags_z_n(res);
        self.s.set_carry(o);
    }
    fn set_flags_z_n_c_o(&mut self, res: u8, o: bool, c: bool) {
        self.set_flags_z_n(res);
        self.s.set_overflow(o);
        self.s.set_carry(c);
    }
    fn set_flags_z_n_o(&mut self, res: u8, m: u8) {
        self.s.set_zero(res == 0);
        self.s.set_negative(m & 0x80 == 0x80);
        self.s.set_overflow(m & 0x40 == 0x40);
    }
}
