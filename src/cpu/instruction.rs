#[derive(Copy, Clone, PartialEq, Debug, Ord, PartialOrd, Eq)]
pub struct Instruction(pub u8);
impl Instruction {
    pub fn get(&self) -> u8 {
        self.0
    }
    pub fn set(&mut self, f: u8) {
        self.0 = f;
    }
    pub fn aaa(&self) -> u8 {
        (self.0 & 0b11100000) >> 5
    }
    pub fn bbb(&self) -> u8 {
        (self.0 &  0b00011100) >> 2
    }
    pub fn cc(&self) -> u8 {
        self.0 & 0b00000011
    }
    pub fn xx(&self) -> u8 {
        (self.0 >> 6) & 0b00000011
    }
    pub fn y(&self) -> bool {
        ((self.0 >> 5) & 0b00000001) == 1
    }
}
