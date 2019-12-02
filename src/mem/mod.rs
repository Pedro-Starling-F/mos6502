pub mod interconnect;
pub use interconnect::Interconnect;

#[derive(Clone)]
pub struct Memory(pub Vec<Box<dyn Interconnect>>);


impl Memory{
    pub fn store8(&mut self, addr:u16, val:u8){
        for v in self.0.iter_mut(){
            v.store8(addr ,val);
        }
    }
    pub fn load8(&self, addr:u16)->u8{
        for v in self.0.iter(){
            if let Some(val) = v.load8(addr){
                return val;
            }
        }
        return 0;
    }
    pub fn load16(&self, addr:u16)->u16{
        let addr2:u16;
        if addr == 0xFF {
            addr2 = 0x0
        }else{
            addr2 = addr+1;
        }
        let b0 = self.load8(addr);
        let b1 = self.load8(addr2);
        return u8_to_u16(&[b0,b1]); 
    }
    pub fn store16(&mut self, addr:u16,val:u16){
        let v = u16_to_u8(val);
        self.store8(addr,v[0]);
        self.store8(addr+1,v[1]);
    }
}

pub fn u16_to_u8(x:u16) -> [u8;2] {
    let b0 : u8 = (x >> 8) as u8;
    let b1 : u8 = x as u8;
    [b1, b0]
}
pub fn u8_to_u16(val:&[u8])->u16{
    let b0 = val[0] as u16;
    let b1 = val[1] as u16;
    (b1 << 8) | (b0 << 0) 
}
