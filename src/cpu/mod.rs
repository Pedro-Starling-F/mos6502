
#![allow(non_snake_case)]
use wasm_bindgen::prelude::*;
mod instruction;
mod flags;
use self::flags::Flags;
use self::instruction::Instruction;

#[wasm_bindgen]
#[derive(Clone)]
pub struct Cpu{
    a: u8,
    x: u8,
    y: u8,
    s: Flags,
    sp: u8,
    pc: u16,
    addr: Option<u16>,
    pub cycles: u8,
    pub in_nmi:bool,
    mem:[u8;0x10000],
    instruction: Instruction,
    max_addr:u16,
}

impl Cpu{
    pub fn new(mem:&[u8]) -> Cpu {
        let instr = Instruction(0xEA);
        let mut memory:[u8;0x10000] = [0;0x10000];
        let max_addr = mem.len() - 1;
        memory[0..=max_addr].copy_from_slice(mem);
        Cpu{
            a: 0,
            x: 0,
            y: 0,
            s: Flags::new(),
            sp: 0xFD,
            pc: 0,
            addr: None,
            cycles: 0,
            in_nmi: false,
            mem:[0;0x10000],
            instruction: instr,
            max_addr:max_addr as u16,
        }
    }
    pub fn irq(&mut self){
        if !self.s.get_interrupt(){
            self.sp-=2;
            let sp = self.sp as u16 + 0x101;
            let pc = self.pc;
            self.store16(sp,pc);
            self.pc = self.load16(0xFFFE);
            self.sp-=1;
            let sp = self.sp as u16 + 0x101;
            let s = self.s.get() | 0b00100000;
            self.store8(sp,s);
        }       
    }
    pub fn nmi(&mut self){
        self.in_nmi = true;
        self.sp-=2;
        let sp = self.sp as u16 + 0x101;
        let pc = self.pc;
        self.store16(sp,pc);
        self.pc = self.load16(0xFFFA);
        self.sp-=1;
        let sp = self.sp as u16 + 0x101;
        let s = self.s.get();
        self.store8(sp,s);
    }
    pub fn load8(&self, addr:u16)->u8{
        let addr = (addr % self.max_addr) as usize;
        self.mem[addr]
    }
    pub fn load16(&self, addr:u16)->u16{
        let addr = (addr % self.max_addr) as usize;
        (self.mem[addr] as u16) << 8 | self.mem[addr+1] as u16
    }
    pub fn store8(&mut self, addr:u16, val:u8){
        let addr = (addr % self.max_addr) as usize;
        self.mem[addr] = val;
    }
    pub fn store16(&mut self,addr:u16,val:u16){
        let addr = (addr % self.max_addr) as usize;
        self.mem[addr] = (val >> 8) as u8;
        self.mem[addr+1] = val as u8;
    }
    pub fn start(&mut self){
        let reset: u16 = self.load16(self.max_addr - 3);
        self.pc = reset;
    }
    pub fn run_next_instruction(&mut self)->fn(&mut Cpu)->fn(&mut Cpu){
        let pc = self.pc;
        //let f = format!("{:04X} A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} ",pc,self.a,self.x,self.y,self.s.get(),self.sp);
        //web_sys::console::log_1(&JsValue::from(f));
        let val = self.load8(pc);
        self.pc+=1;
        self.cycles+=2;
        self.instruction.set(val);
        return Cpu::decode_and_execute;
    }
    pub fn decode_and_execute(&mut self)->fn(&mut Cpu){
        match self.instruction.get(){
            0x00 => Cpu::BRK,
            0x08 => Cpu::PHP,
            0x18 => Cpu::CLC,
            0x20 =>{self.addr=Some(self.absolute());
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
            0xF8 => Cpu::SED,
            _ => match self.instruction.cc() {
                0 =>{
                    if let Some(i) = self.addressing0(){
                        self.addr = Some(i);
                        match self.instruction.aaa() {
                            1 => Cpu::BIT,
                            2 => Cpu::JMP,
                            3 => Cpu::JMI,
                            4 => Cpu::STY,
                            5 => Cpu::LDY,
                            6 => Cpu::CPY,
                            7 => Cpu::CPX,
                            _ => {web_sys::console::log_1(&JsValue::from("INVALID ADDRESSING MODE 0"));Cpu::NOP},
                        }
                    }else{
                        Cpu::relative
                    }
                },
                1 =>{
                    self.addressing1();  
                    match self.instruction.aaa() {
                        0 => Cpu::ORA,
                        1 => Cpu::AND,
                        2 => Cpu::EOR,
                        3 => Cpu::ADC,
                        4 => Cpu::STA,
                        5 => Cpu::LDA,
                        6 => Cpu::CMP,
                        7 => Cpu::SBC,
                        _ => {web_sys::console::log_1(&JsValue::from("INVALID ADDRESSING MODE 1"));Cpu::NOP},
                    }
                },
                2 =>{
                    self.addressing2();
                    match self.instruction.aaa() {
                        0 => Cpu::ASL,
                        1 => Cpu::ROL,
                        2 => Cpu::LSR,
                        3 => Cpu::ROR,
                        4 => Cpu::STX,
                        5 => Cpu::LDX,
                        6 => Cpu::DEC,
                        7 => Cpu::INC,
                        _ => {web_sys::console::log_1(&JsValue::from("INVALID ADDRESSING MODE 2"));Cpu::NOP},
                    }
                },
                _ => {web_sys::console::log_1(&JsValue::from("INVALID INSTRUCTION"));Cpu::NOP},
            }
        }
    }
    fn addressing0(&mut self) -> Option<u16>{
        match self.instruction.bbb() {
            0 => Some(self.immediate()),
            1 => Some(self.zero_page()),
            3 => Some(self.absolute()),
            4 => None,
            5 => Some(self.zero_page_r()),
            7 => Some(self.absolute_x()),
            _ => {web_sys::console::log_1(&JsValue::from("UNHANDLED ADDRESSING MODE 0"));None},
        }
    }
    fn addressing1(&mut self){
        self.addr = Some(match self.instruction.bbb() {
            0 => self.indexed_indirect(),
            1 => self.zero_page(),
            2 => self.immediate(),
            3 => self.absolute(),
            4 => self.indirect_indexed(),
            5 => self.zero_page_r(),
            6 => self.absolute_y(),
            7 => self.absolute_x(),
            _ => {web_sys::console::log_1(&JsValue::from("UNHANDLED ADDRESSING MODE 1"));0},
        })
    }
    fn addressing2(&mut self){
        self.addr = match self.instruction.bbb() {
            0 => Some(self.immediate()),
            1 => Some(self.zero_page()),
            2 => None,
            3 => Some(self.absolute()),
            5 => Some(self.zero_page_r2()),
            7 => Some(match self.instruction.aaa(){
                        5 => self.absolute_y(),
                        _ => self.absolute_x(),
                }),
            _ => {web_sys::console::log_1(&JsValue::from("UNHANDLED ADDRESSING MODE 2"));None},
        }
    }
    fn indirect_indexed(&mut self) -> u16 {
        let pc = self.pc;
        let id = self.load8(pc);
        self.pc+=1;
        let idix = self.load16(id as u16);
        let idixy = idix.wrapping_add(self.y as u16);
        self.cycles+=4;
        idixy
    }
    fn indexed_indirect(&mut self) -> u16 {
        let pc = self.pc;
        let x = self.x;
        let id = self.load8(pc).wrapping_add(x);
        self.pc+=1;
        let ixid = self.load16(id as u16);
        self.cycles+=4;
        ixid
    }
    fn immediate(&mut self) -> u16 {
        self.pc+=1;
        self.pc - 1
    }
    fn zero_page(&mut self) -> u16 {
        let pc = self.pc;
        self.cycles+=1;
        let zp = self.load8(pc);
        self.pc+=1;
        zp as u16
    }
    fn zero_page_r(&mut self) -> u16 {
        let pc = self.pc;
        let zpr = self.load8(pc);
        self.pc+=1;
        self.cycles+=2;
        zpr.wrapping_add(self.x) as u16
    }
    fn zero_page_r2(&mut self) -> u16 {
        let pc = self.pc;
        let zpr = self.load8(pc);
        self.pc+=1;
        self.cycles+=2;
        match self.instruction.aaa(){
            4 | 5 =>  zpr.wrapping_add(self.y) as u16,
            _ => zpr.wrapping_add(self.x) as u16,
        }
    }
    fn absolute(&mut self) -> u16 {
        let pc = self.pc;
        let id = self.load16(pc);
        self.pc+=2;
        self.cycles+=2;
        id
    }
    fn absolute_x(&mut self) -> u16 {
        let pc = self.pc;
        let mut idx = self.load16(pc);
        idx += self.x as u16;
        self.pc+=2;
        self.cycles+=3;
        idx
    }
    fn absolute_y(&mut self) -> u16 {
        let pc = self.pc;
        let mut idy = self.load16(pc);
        idy = idy.wrapping_add(self.y as u16);
        self.pc+=2;
        self.cycles+=3;
        idy
    }
    fn relative(&mut self){
        match self.instruction.xx(){
            0 => if self.s.get_negative() == self.instruction.y() {self.branch()},
            1 => if self.s.get_overflow() == self.instruction.y() {self.branch()},
            2 => if self.s.get_carry() == self.instruction.y() {self.branch()},
            3 => if self.s.get_zero() == self.instruction.y() {self.branch()},
            _ => web_sys::console::log_1(&JsValue::from("UNHANDLED BRANCH")),
        };
        self.pc+=1;
    }
    fn branch(&mut self) {
        let pc = self.pc;
        let offset = self.load8(pc)as i8;
        self.pc = (self.pc as i32).wrapping_add(offset as i32) as u16;
        if (pc/256)!=(self.pc/256) {
            self.cycles+=2;
        }
        self.cycles+=1;
    }
    fn ORA(&mut self){
        let addr = self.addr;
        let m = self.load8(addr.unwrap());
        let a = self.a|m;
        self.set_flags_z_n(a);
        self.a = a;
    }
    fn AND(&mut self){
        let addr = self.addr;
        let m = self.load8(addr.unwrap());
        let a= self.a&m;
        self.set_flags_z_n(a);
        self.a = a;
    }
    fn EOR(&mut self){
        let addr = self.addr;
        let m = self.load8(addr.unwrap());
        let a = self.a^m;
        self.a = a;
        self.set_flags_z_n(a);
    }
    fn ADC(&mut self){
        let addr = self.addr;
        let m = self.load8(addr.unwrap()) as i8;
        let c = self.s.get_carry() as i8;
        let (mc,o) = m.overflowing_add(c);
        let (_,c) = (self.a as i8).overflowing_add(mc);
        let o = c||o;
        let (a,c) = self.a.overflowing_add(mc as u8);
        self.a = a as u8;
        self.set_flags_z_n_c_o(a,o,c);
    }
    fn STA(&mut self){
        let addr = self.addr;
        let a = self.a;
        self.store8(addr.unwrap(),a);
    }
    fn LDA(&mut self){
        let addr = self.addr;
        let val = self.load8(addr.unwrap());
        self.set_flags_z_n(val);
        self.a = val;
    }
    fn CMP(&mut self){
        let addr = self.addr;
        let val = self.load8(addr.unwrap());
        let (res,o) = self.a.overflowing_sub(val);
        self.set_flags_z_n_c(res,!o);
    }
    fn SBC(&mut self){
        let addr = self.addr;
        let m = self.load8(addr.unwrap());
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
    fn ASL(&mut self){
        if let Some(addr) = self.addr{
            let mut m = self.load8(addr);
            let m2 = m;
            m = m<<1;
            self.store8(addr,m);
            self.set_flags_z_n_c(m,m2&0x80==0x80);
        }else{
            let a1 = self.a<<1;
            let a2 = self.a;
            self.a = a1;
            self.set_flags_z_n_c(a1,a2&0x80==0x80);
        }
    }
    fn ROL(&mut self){
        if let Some(addr) = self.addr{
            let m = self.load8(addr);
            let m2 = m;
            let (m,_) = m.overflowing_mul(2);
            let (m,_) = m.overflowing_add(self.s.get_carry() as u8);
            self.store8(addr,m);
            self.set_flags_z_n_c(m,m2&0x80==0x80);
        }else{
            let (a1,_) = self.a.overflowing_mul(2);
            let (a1,_) = a1.overflowing_add(self.s.get_carry() as u8 );
            let a2 = self.a;
            self.a = a1;
            self.set_flags_z_n_c(a1,a2&0x80==0x80);
        }
    }
    fn LSR(&mut self){
        if let Some(addr) = self.addr{
            let mut m = self.load8(addr);
            let m2 = m;
            m = m>>1;
            self.store8(addr,m);
            self.set_flags_z_n_c(m,m2&1==1);
        }else{
            let a1 = self.a>>1;
            let a2 = self.a;
            self.a = a1;
            self.set_flags_z_n_c(a1,a2&1==1);
        }
    }
    fn ROR(&mut self){
        if let Some(addr) = self.addr{
            let mut m = self.load8(addr);
            let m2 = m;
            m = m/2 + ((self.s.get_carry() as u8) << 7);
            self.store8(addr,m);
            self.set_flags_z_n_c(m,m2&1==1);
        }else{
            let a1 = self.a/2 + ((self.s.get_carry() as u8) << 7);
            let a2 = self.a;
            self.a = a1 as u8;
            self.set_flags_z_n_c(a1 as u8,a2&1==1);
        }
    }
    fn STX(&mut self){
        if let Some(addr) = self.addr{
            let x = self.x;
            self.store8(addr, x);
        }else{
            web_sys::console::log_1(&JsValue::from("UNHANDLED ADDR IN STX"));
        }
    }
    fn LDX(&mut self){
        if let Some(addr) = self.addr{
        let val = self.load8(addr);
        self.set_flags_z_n(val);
        self.x = val;
        }else{
            web_sys::console::log_1(&JsValue::from("UNHANDLED ADDR IN LDX"));
        }
    }
    fn DEC(&mut self){
        if let Some(addr) = self.addr{
            let m = self.load8(addr).wrapping_sub(1);
            self.store8(addr,m);
            self.set_flags_z_n(m);
        }else{
            web_sys::console::log_1(&JsValue::from("UNHANDLED ADDR IN DEC"));
        }
    }
    fn INC(&mut self){
        if let Some(addr) = self.addr{
            let m = self.load8(addr).wrapping_add(1);
            self.store8(addr,m);
            self.set_flags_z_n(m);
        }else{
            web_sys::console::log_1(&JsValue::from("UNHANDLED ADDR IN INC"));
        }
    }
    fn BIT(&mut self){
        let addr = self.addr;
        let m = self.load8(addr.unwrap());
        let res = self.a&m;
        self.set_flags_z_n_o(res,m);
    }
    fn JMP(&mut self){ 
        self.pc = self.addr.unwrap();
    }
    fn JMI(&mut self){
        let addr = self.addr;
        let valL = self.load8(addr.unwrap());
        let addrH = self.addr.unwrap() & 0xFF00;
        let addrL = self.addr.unwrap() as u8;
        let addr = addrH | addrL.wrapping_add(1) as u16; 
        let valH = self.load8(addr);
        self.pc = valL as u16 | (valH as u16)<<8;
    }
    fn STY(&mut self){
        let y = self.y;
        let addr = self.addr;
        self.store8(addr.unwrap(),y);
    }
    fn LDY(&mut self){
        let addr = self.addr;
        let m = self.load8(addr.unwrap());
        self.y = m;
        self.set_flags_z_n(m);
    }
    fn CPY(&mut self){
        let addr = self.addr;
        let val = self.load8(addr.unwrap());
        let y = self.y;
        let (res,o) = y.overflowing_sub(val);
        self.set_flags_z_n_c(res,!o);
    }
    fn CPX(&mut self){
        let addr = self.addr;
        let val = self.load8(addr.unwrap());
        let x = self.x;
        let (res,o) = x.overflowing_sub(val);
        self.set_flags_z_n_c(res,!o);
    }
    fn BRK(&mut self){
        self.cycles+=5;
        self.irq();
    }
    fn JSR(&mut self){
        self.sp-=2;
        let sp = self.sp as u16 + 0x101;
        let pc = self.pc-1;
        self.store16(sp, pc);
        self.pc = self.addr.unwrap();
        self.cycles+=2;
    }
    fn RTI(&mut self){
        self.in_nmi = false;
        self.cycles+=4;
        let sp = self.sp as u16 + 0x101;
        let s = self.load8(sp) & 0b11001111;
        self.sp +=3;
        self.s.set(s);
        self.pc = self.load16(sp+1);
        self.cycles+=4;
    }
    fn RTS(&mut self){
        self.cycles+=4;
        let s = self.sp as u16 + 0x101;
        self.pc = self.load16(s)+1;
        self.sp+=2;
    }
    fn PHP(&mut self){
        self.sp-=1;
        let sp = self.sp as u16 + 0x101;
        let s = self.s.get() | 0b0110000;
        self.store8(sp, s);
        self.cycles+=1;
    }
    fn PLP(&mut self){
        let s:u16 = self.sp as u16 + 0x101;
        let p = self.load8(s) & 0b11001111;
        self.s.set(p);
        self.sp+=1;
        self.cycles+=2;
    }
    fn PHA(&mut self){
        self.sp-=1;
        let a = self.a;
        let sp = self.sp as u16 + 0x101;
        self.store8(sp, a);
        self.cycles+=1;
    }
    fn PLA(&mut self){
        let s = self.sp as u16 + 0x101;
        let a = self.load8(s); 
        self.a = a;
        self.sp+=1;
        self.set_flags_z_n(a);
        self.cycles+=2;
    }
    fn DEY(&mut self){
        let y = self.y.wrapping_sub(1);
        self.y = y;
        self.set_flags_z_n(y);
    }
    fn TAY(&mut self){
        let a = self.a;
        self.y = a;
        self.set_flags_z_n(a);
    }
    fn INY(&mut self){
        let y = self.y.wrapping_add(1);
        self.y = y;
        self.set_flags_z_n(y);
    }
    fn INX(&mut self){
        let x = self.x.wrapping_add(1);
        self.x = x;
        self.set_flags_z_n(x);
    }
    fn CLC(&mut self){
        self.s.set_carry(false);
    }
    fn SEC(&mut self){
        self.s.set_carry(true);
    }
    fn CLI(&mut self){
        self.s.set_interrupt(false);
    }
    fn SEI(&mut self){
        self.s.set_interrupt(true);
    }
    fn TYA(&mut self){
        let y = self.y;
        self.a = y;
        self.set_flags_z_n(y);
    }
    fn CLV(&mut self){
        self.s.set_overflow(false);
    }
    fn CLD(&mut self){
        self.s.set_decimal(false);
    }
    fn SED(&mut self){
        self.s.set_decimal(true);
    }
    fn TXA(&mut self){
        let x = self.x;
        self.a = x;
        self.set_flags_z_n(x);
    }
    fn TXS(&mut self){
        self.sp = self.x;
    }
    fn TAX(&mut self){
        let a = self.a;
        self.x = a;
        self.set_flags_z_n(a);
    }
    fn TSX(&mut self){
        let s = self.sp;
        self.x = s;
        self.set_flags_z_n(s);}
    fn DEX(&mut self){
        let x = self.x.wrapping_sub(1);
        self.x = x;
        self.set_flags_z_n(x);}
    fn NOP(&mut self){}
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