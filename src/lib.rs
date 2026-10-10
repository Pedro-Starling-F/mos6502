//#![cfg_attr(not(feature = "std"), no_std)]
pub mod cpu;
pub use cpu::Cpu;

mod tests;

// nestest checks that ADC/SBC ignore the decimal flag, as on the NES 2A03
#[cfg(all(test, feature = "std", not(feature = "decimal")))]
mod test {
    use super::*;
    use core::ops::{Index, IndexMut};
    use std::fs;
    use std::io::Write;

    // APU/IO registers aren't emulated; nestest's log reads them as $FF
    static OPEN_BUS: u8 = 0xFF;

    pub struct NesMemory {
        pub ram: [u8; 2048],
        pub rom: [u8; 16384],
        pub dummy: u8,
    }
    impl Index<u16> for NesMemory {
        type Output = u8;
        fn index(&self, index: u16) -> &Self::Output {
            match index {
                // 2 KB of RAM mirrored up to $1FFF; one 16 KB PRG bank mirrored at $8000 and $C000
                0x0000..=0x1FFF => &self.ram[(index & 0x07FF) as usize],
                0x8000..=0xFFFF => &self.rom[(index & 0x3FFF) as usize],
                _ => &OPEN_BUS,
            }
        }
    }
    impl IndexMut<u16> for NesMemory {
        fn index_mut(&mut self, index: u16) -> &mut Self::Output {
            match index {
                0x0000..=0x1FFF => &mut self.ram[(index & 0x07FF) as usize],
                // writes to ROM and unmapped registers are dropped
                _ => &mut self.dummy,
            }
        }
    }

    /// Runs nestest.nes in automated mode (from $C000) and compares every
    /// instruction against the reference log: PC, opcode, registers and the
    /// cycle count since power-on. Needs nestest.nes and correct_log.txt
    /// (the standard nestest.log) in the crate root.
    #[test]
    pub fn main() {
        let correct_log = fs::read_to_string("correct_log.txt").unwrap();
        let mut log_file = fs::File::create("log.txt").unwrap();
        let file = fs::read("nestest.nes").unwrap();
        let mut rom = [0u8; 0x4000];
        rom.copy_from_slice(&file[0x0010..0x4010]);
        let mut mem = NesMemory { ram: [0; 0x0800], rom, dummy: 0 };
        let mut core = Cpu::new(Some(0xC000));
        // reset takes 7 cycles before the first instruction
        let mut total_cycles: isize = 7;
        for (i, expected) in correct_log.lines().enumerate() {
            let fetch_cycles = core.run(&mut mem);
            let got = format!("{} CYC:{}", core.log_line, total_cycles);
            writeln!(log_file, "{}", got).unwrap();
            let want_cycles: isize = expected[expected.find("CYC:").unwrap() + 4..].trim().parse().unwrap();
            if got[0..8] != expected[0..8] || got[48..73] != expected[48..73] || total_cycles != want_cycles {
                panic!("nestest mismatch on line {}
want: {}
got:  {}", i + 1, expected, got);
            }
            total_cycles += fetch_cycles + core.run(&mut mem) + core.run(&mut mem);
        }
        // nestest stores its error codes at $02/$03; zero means every check passed
        assert_eq!((mem[0x02], mem[0x03]), (0, 0), "nestest reported errors");
    }
}
