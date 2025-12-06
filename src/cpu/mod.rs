#![allow(non_snake_case)]

mod instruction;
mod flags;
use core::ops::{IndexMut, Index};
use core::cmp::{PartialEq, Eq};
use self::flags::Flags;
use self::instruction::Instruction;
use States::*;
#[cfg(feature = "logging")]
use log::*;

#[derive(PartialEq, Clone, Copy, Eq,  Ord, PartialOrd, Debug)]
enum States{
    Fetch,
    Decode,
    Execute,
}

#[derive(Clone,Ord, PartialOrd, Debug)]
pub struct Cpu{
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
    pub in_nmi:bool,
    pub instruction: Instruction,
    states:States,
    current_instr:fn(&mut Cpu,  &mut dyn IndexMut<u16, Output=u8>),
}
impl PartialEq for Cpu {
    fn eq(&self, other: &Cpu) -> bool {
        self.a == other.a && self.x == other.x && self.y == other.y && self.s == other.s && self.sp == other.sp && self.pc == other.pc
    }
}
impl Eq for Cpu {}
impl Cpu{
    pub fn new_test(pc:u16,s:u8,a:u8,x:u8,y:u8,p:u8)-> Cpu{
        let mut status = Flags::new();
        status.set(p);
        Cpu{
            a,
            x,
            y,
            s:status,
            sp:s,
            pc,
            addr: None,
            cycles:0,
            in_nmi:false,
            instruction:Instruction(0xEA),
            states: Fetch,
            current_instr:Cpu::NOP
        }
    }
    pub fn new(init_pc:Option<u16>) -> Cpu {
        let i_pc:u16;
        if let Some(pc) = init_pc {
            i_pc = pc;
        }else{
            i_pc = 0;
        }
        Cpu{
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
            states:Fetch,
            current_instr:Cpu::NOP
        }
    }
    pub fn load16(&self, mem: &mut dyn IndexMut<u16, Output=u8>,addr:u16)->u16{
        let addr2:u16;
        if addr == 0xFF {
            addr2 = 0x0
        }else{
            addr2 = addr+1;
        }
        let b0 = mem[addr];
        let b1 = mem[addr2];
        u16::from_le_bytes([b0,b1])
    }
    pub fn store16(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>, addr:u16,val:u16){
        let v = val.to_le_bytes();
        mem[addr] = v[0];
        mem[addr+1] = v[1];
    }
    pub fn irq(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        if !self.s.get_interrupt(){
            self.sp = self.sp.wrapping_sub(2);
            let sp = self.sp as u16 + 0x100;
            let pc = self.pc;
            self.store16(mem, sp, pc);
            self.pc = self.load16(mem, 0xFFFE);
            self.sp = self.sp.wrapping_sub(1);
            let sp = self.sp as u16 + 0x100;
            let s = self.s.get();
            mem[sp] = s;
        }       
    }
    pub fn nmi(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.in_nmi = true;
        self.sp = self.sp.wrapping_sub(2);
        let sp = self.sp as u16 + 0x100;
        let pc = self.pc;
        self.store16(mem, sp, pc);
        self.pc = self.load16(mem, 0xFFFE);
        self.sp = self.sp.wrapping_sub(1);
        let sp = self.sp as u16 + 0x100;
        let s = self.s.get();
        mem[sp] = s;
    }
    pub fn start(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.cycles += 6;
        let reset: u16 = self.load16(mem,0xFFFC);
        self.pc = reset;
    }
    pub fn run_instr(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let pc = self.pc;
        let val = mem[pc];
        self.pc+=1;
        self.instruction.set(val);
        self.current_instr = self.decode(mem);
        (self.current_instr)(self, mem);
        //self.cycles+=1;
    }
    pub fn run(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>)->isize{
        self.cycles = 0;
        match self.states{
            Fetch => {self.fetch(mem);
                      self.states = Decode;},
            Decode => {self.current_instr=self.decode(mem);
                       self.states = Execute;},
            Execute =>{(self.current_instr)(self, mem);
                        self.states = Fetch;}
        }
        self.cycles
    }
    fn fetch(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let pc = self.pc;
        let val = mem[pc];
        self.pc+=1;
        self.cycles+=1;
        self.instruction.set(val);
        //self.log_line = format!("{:04X}  {:02X}                                        A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} CYC:{:}\n",pc,val,self.a,self.x,self.y,self.s.get(),self.sp, self.cycles);
        //trace!("{:04X}  {:02X}        A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} CYC:{:08X}",pc,val,self.a,self.x,self.y,self.s.get(),self.sp, self.cycles);
    }
    fn decode(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>)->fn(&mut Cpu,  &mut dyn IndexMut<u16, Output=u8>){
        self.cycles+=1;
        match self.instruction.get(){
            0x00 => Cpu::BRK,
            0x02 | 0x12 | 0x22 | 0x32 | 0x42 | 0x52 | 0x62 | 0x72 | 0x92 | 0xB2 | 0xD2 | 0xF2 => Cpu::JAM,
            0x08 => Cpu::PHP,
            0x18 => Cpu::CLC,
            0x20 =>{self.addr=Some(self.absolute(mem));
                    Cpu::JSR},
            0x28 => Cpu::PLP,
            0x38 => Cpu::SEC,
            0x40 => Cpu::RTI,
            0x48 => Cpu::PHA,
            0x58 => Cpu::CLI,
            0x60 => Cpu::RTS,
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
            0xEA => Cpu::NOP,
            0x04 | 0x44 | 0x64 => {self.pc += 1; Cpu::NOP},
            0x0C => {self.pc += 2; Cpu::NOP},

            0xF8 => Cpu::SED,
            _ => match self.instruction.cc() {
                0 =>{
                    if let Some(i) = self.addressing0(mem){
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
                    }else{
                        Cpu::relative
                    }
                },
                1 =>{
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
                },
                2 =>{
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
                },
                _ => panic!(),
            }
        }
    }
    fn addressing0(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>) -> Option<u16>{
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
    fn addressing1(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.addr = Some(match self.instruction.bbb() {
            0 => self.indexed_indirect(mem),
            1 => self.zero_page(mem),
            2 => self.immediate(),
            3 => self.absolute(mem),
            4 => self.indirect_indexed(mem),
            5 => self.zero_page_r(mem),
            6 => self.absolute_y(mem),
            7 => self.absolute_x(mem),
            _ => 0,
        })
    }
    fn addressing2(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.addr = match self.instruction.bbb() {
            0 => Some(self.immediate()),
            1 => Some(self.zero_page(mem)),
            2 => None,
            3 => Some(self.absolute(mem)),
            5 => Some(self.zero_page_r2(mem)),
            7 => Some(match self.instruction.aaa(){
                        5 => self.absolute_y(mem),
                        _ => self.absolute_x(mem),
                }),
            _ => None,
        }
    }
    fn indirect_indexed(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>) -> u16 {
        let pc = self.pc;
        let id = mem[pc];
        let idix = self.load16(mem, id as u16);
        let idixy = idix.wrapping_add(self.y as u16);
        self.cycles+=4;
        self.pc += 1;
        idixy
    }
    fn indexed_indirect(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>) -> u16 {
        let pc = self.pc;
        let x = self.x;
        let id = mem[pc].wrapping_add(x);
        self.pc+=1;
        let ixid =self.load16(mem, id as u16);
        self.cycles+=4;
        ixid
    }
    fn immediate(&mut self) -> u16 {
        self.pc+=1;
        self.pc - 1
    }
    fn zero_page(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>) -> u16 {
        let pc = self.pc;
        self.cycles+=1;
        let zp = mem[pc];
        self.pc+=1;
        zp as u16
    }
    fn zero_page_r(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>) -> u16 {
        let pc = self.pc;
        let zpr = mem[pc];
        self.pc+=1;
        self.cycles+=2;
        zpr.wrapping_add(self.x) as u16
    }
    fn zero_page_r2(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>) -> u16 {
        let pc = self.pc;
        let zpr = mem[pc];
        self.pc+=1;
        self.cycles+=2;
        match self.instruction.aaa(){
            4 | 5 =>  zpr.wrapping_add(self.y) as u16,
            _ => zpr.wrapping_add(self.x) as u16,
        }
    }
    fn absolute(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>) -> u16 {
        let pc = self.pc;
        let id = self.load16(mem, pc);
        self.pc+=2;
        self.cycles+=2;
        id
    }
    fn absolute_x(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>) -> u16 {
        let pc = self.pc;
        let mut idx = self.load16(mem, pc);
        idx += self.x as u16;
        self.pc+=2;
        self.cycles+=3;
        idx
    }
    fn absolute_y(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>) -> u16 {
        let pc = self.pc;
        let mut idy = self.load16(mem, pc);
        idy = idy.wrapping_add(self.y as u16);
        self.pc+=2;
        self.cycles+=3;
        idy
    }
    fn relative(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        match self.instruction.xx(){
            0 => if self.s.get_negative() == self.instruction.y() {self.branch(mem)},
            1 => if self.s.get_overflow() == self.instruction.y() {self.branch(mem)},
            2 => if self.s.get_carry() == self.instruction.y() {self.branch(mem)},
            3 => if self.s.get_zero() == self.instruction.y() {self.branch(mem)},
            _ =>{},
        };
        self.pc+=1;
    }
    fn branch(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>) {
        let pc = self.pc;
        let offset = mem[pc]as i8;
        self.pc = (self.pc as i32).wrapping_add(offset as i32) as u16;
        if (pc/256)!=(self.pc/256) {
            self.cycles+=2;
        }
        self.cycles+=1;
    }
    fn JAM(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){

    }
    fn ORA(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let m = mem[addr.unwrap()];
        let a = self.a|m;
        self.set_flags_z_n(a);
        self.a = a;
    }
    fn AND(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let m = mem[addr.unwrap()];
        let a= self.a & m;
        self.set_flags_z_n(a);
        self.a = a;
    }
    fn EOR(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let m = mem[addr.unwrap()];
        let a = self.a^m;
        self.a = a;
        self.set_flags_z_n(a);
    }
    fn ADC(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let m = mem[addr.unwrap()] as i8;
        let c = self.s.get_carry() as i8;
        let (mc,o) = m.overflowing_add(c);
        let (_,c) = (self.a as i8).overflowing_add(mc);
        let o = c||o;
        let (a,c) = self.a.overflowing_add(mc as u8);
        self.a = a as u8;
        self.set_flags_z_n_c_o(a,o,c);
    }
    fn STA(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let a = self.a;
        mem[addr.unwrap()] = a;
    }
    fn LDA(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let val = mem[addr.unwrap()];
        self.set_flags_z_n(val);
        self.a = val;
    }
    fn CMP(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let val = mem[addr.unwrap()];
        let (res,o) = self.a.overflowing_sub(val);
        self.set_flags_z_n_c(res,!o);
    }
    fn SBC(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let m = mem[addr.unwrap()];
        let cf = self.s.get_carry() as u8;
        let (mc,o) = (self.a as i8).overflowing_sub(m as i8);
        let (_,of) = mc.overflowing_sub(1-cf as i8);
        let (a,c) = self.a.overflowing_sub(m);
        let (a,cr) = a.overflowing_sub(1-cf);
        let o = o||of;
        let c = !(c||cr);
        self.a = a as u8;
        self.set_flags_z_n_c_o(a,o,c);
    }
    fn ASL(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        if let Some(addr) = self.addr{
            let mut m = mem[addr];
            let m2 = m;
            m = m<<1;
            mem[addr] = m;
            self.set_flags_z_n_c(m,m2&0x80==0x80);
        }else{
            let a1 = self.a<<1;
            let a2 = self.a;
            self.a = a1;
            self.set_flags_z_n_c(a1,a2&0x80==0x80);
        }
    }
    fn ROL(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        if let Some(addr) = self.addr{
            let m = mem[addr];
            let m2 = m;
            let (m,_) = m.overflowing_mul(2);
            let (m,_) = m.overflowing_add(self.s.get_carry() as u8);
            mem[addr]= m;
            self.set_flags_z_n_c(m,m2&0x80==0x80);
        }else{
            let (a1,_) = self.a.overflowing_mul(2);
            let (a1,_) = a1.overflowing_add(self.s.get_carry() as u8 );
            let a2 = self.a;
            self.a = a1;
            self.set_flags_z_n_c(a1,a2&0x80==0x80);
        }
    }
    fn LSR(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        if let Some(addr) = self.addr{
            let mut m = mem[addr];
            let m2 = m;
            m = m>>1;
            mem[addr]=m;
            self.set_flags_z_n_c(m,m2&1==1);
        }else{
            let a1 = self.a>>1;
            let a2 = self.a;
            self.a = a1;
            self.set_flags_z_n_c(a1,a2&1==1);
        }
    }
    fn ROR(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        if let Some(addr) = self.addr{
            let mut m = mem[addr];
            let m2 = m;
            m = m/2 + ((self.s.get_carry() as u8) << 7);
            mem[addr] = m;
            self.set_flags_z_n_c(m,m2&1==1);
        }else{
            let a1 = self.a/2 + ((self.s.get_carry() as u8) << 7);
            let a2 = self.a;
            self.a = a1 as u8;
            self.set_flags_z_n_c(a1 as u8,a2&1==1);
        }
    }
    fn STX(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        if let Some(addr) = self.addr{
            let x = self.x;
            mem[addr] = x;
        }
    }
    fn LDX(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        if let Some(addr) = self.addr{
        let val = mem[addr];
        self.set_flags_z_n(val);
        self.x = val;
        }
    }
    fn DEC(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        if let Some(addr) = self.addr{
            let m = mem[addr].wrapping_sub(1);
            mem[addr] = m;
            self.set_flags_z_n(m);
        }
    }
    fn INC(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        if let Some(addr) = self.addr{
            let m = mem[addr].wrapping_add(1);
            mem[addr] = m;
            self.set_flags_z_n(m);
        }
    }
    fn BIT(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let m = mem[addr.unwrap()];
        let res = self.a&m;
        self.set_flags_z_n_o(res,m);
    }
    fn JMP(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.pc = self.addr.unwrap();
        self.cycles += 2;
    }
    fn JMI(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let valL = mem[addr.unwrap()];
        let addrH = self.addr.unwrap() & 0xFF00;
        let addrL = self.addr.unwrap() as u8;
        let addr2 = addrH | addrL.wrapping_add(1) as u16;
        let valH = mem[addr2];
        self.pc = valL as u16 | (valH as u16)<<8;
    }
    fn STY(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let y = self.y;
        let addr = self.addr;
        mem[addr.unwrap()] = y;
    }
    fn LDY(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let m = mem[addr.unwrap()];
        self.y = m;
        self.set_flags_z_n(m);
    }
    fn CPY(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let val = mem[addr.unwrap()];
        let y = self.y;
        let (res,o) = y.overflowing_sub(val);
        self.set_flags_z_n_c(res,!o);
    }
    fn CPX(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let addr = self.addr;
        let val = mem[addr.unwrap()];
        let x = self.x;
        let (res,o) = x.overflowing_sub(val);
        self.set_flags_z_n_c(res,!o);
    }
    fn BRK(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.cycles+=5;
        self.sp = self.sp.wrapping_sub(2);
        let sp = self.sp as u16 + 0x100;
        let pc = self.pc;
        self.store16(mem, sp, pc);
        self.pc = self.load16(mem, 0xFFFE);
        self.sp = self.sp.wrapping_sub(1);
        let sp = self.sp as u16 + 0x100;
        let s = self.s.get() | 0b00110100;
        self.s.set_interrupt(true);
        mem[sp] = s;
    }
    fn JSR(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.sp = self.sp.wrapping_sub(2);
        let sp = self.sp as u16 + 0x100;
        let pc = self.pc-1;
        self.store16(mem, sp+1, pc);
        self.pc = self.addr.unwrap();
        self.cycles+=2;
    }
    fn RTI(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.in_nmi = false;
        self.cycles+=4;
        let sp = self.sp as u16 + 0x100;
        let s = mem[sp+1];
        self.s.set((s | 0x20) & 0xEF);
        self.pc = self.load16(mem, sp+2);
        self.sp = self.sp.wrapping_add(3);
        self.cycles+=4;
    }
    fn RTS(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.cycles+=4;
        let sp = self.sp as u16 + 0x100;
        self.pc = self.load16(mem, sp+1)+1;
        self.sp = self.sp.wrapping_add(2);
    }
    fn PHP(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.sp = self.sp.wrapping_sub(1);
        let sp = self.sp as u16 + 0x100;
        let s = self.s.get() | 0x10;
        mem[sp+1] = s;
        self.cycles+=1;
    }
    fn PLP(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let sp:u16 = self.sp as u16 + 0x100;
        let p = mem[sp+1];
        self.s.set((p | 0x20) & 0xEF);
        self.sp = self.sp.wrapping_add(1);
        self.cycles+=2;
    }
    fn PHA(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.sp = self.sp.wrapping_sub(1);
        let a = self.a;
        let sp = self.sp as u16 + 0x100;
        mem[sp+1] = a;
        self.cycles+=1;
    }
    fn PLA(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let sp = self.sp as u16 + 0x100;
        let a = mem[sp+1];
        self.a = a;
        self.sp = self.sp.wrapping_add(1);
        self.set_flags_z_n(a);
        self.cycles+=2;
    }
    fn DEY(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let y = self.y.wrapping_sub(1);
        self.y = y;
        self.set_flags_z_n(y);
    }
    fn TAY(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let a = self.a;
        self.y = a;
        self.set_flags_z_n(a);
    }
    fn INY(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let y = self.y.wrapping_add(1);
        self.y = y;
        self.set_flags_z_n(y);
    }
    fn INX(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let x = self.x.wrapping_add(1);
        self.x = x;
        self.set_flags_z_n(x);
    }
    fn CLC(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.s.set_carry(false);
    }
    fn SEC(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.s.set_carry(true);
    }
    fn CLI(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.s.set_interrupt(false);
    }
    fn SEI(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.s.set_interrupt(true);
    }
    fn TYA(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let y = self.y;
        self.a = y;
        self.set_flags_z_n(y);
    }
    fn CLV(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.s.set_overflow(false);
    }
    fn CLD(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.s.set_decimal(false);
    }
    fn SED(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.s.set_decimal(true);
    }
    fn TXA(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let x = self.x;
        self.a = x;
        self.set_flags_z_n(x);
    }
    fn TXS(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        self.sp = self.x;
    }
    fn TAX(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let a = self.a;
        self.x = a;
        self.set_flags_z_n(a);
    }
    fn TSX(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let s = self.sp;
        self.x = s;
        self.set_flags_z_n(s);
    }
    fn DEX(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){
        let x = self.x.wrapping_sub(1);
        self.x = x;
        self.set_flags_z_n(x);
    }
    fn NOP(&mut self, mem: &mut dyn IndexMut<u16, Output=u8>){}
    fn set_flags_z_n(&mut self,res:u8){
        self.s.set_zero(res == 0);
        self.s.set_negative(res & 0x80 == 0x80 );
    }
    fn set_flags_z_n_c(&mut self,res:u8,o:bool){
        self.set_flags_z_n(res);
        self.s.set_carry(o);
    }
    fn set_flags_z_n_c_o(&mut self,res:u8,o:bool,c:bool){
        self.set_flags_z_n(res);
        self.s.set_overflow(o);
        self.s.set_carry(c);
    }
    fn set_flags_z_n_o(&mut self,res:u8,m:u8){
        self.s.set_zero(res == 0);
        self.s.set_negative(m & 0x80 == 0x80);
        self.s.set_overflow(m & 0x40 == 0x40);
    }
}


