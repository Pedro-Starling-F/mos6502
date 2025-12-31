pub mod single_step;
use crate::Cpu;
use core::ops::{Index, IndexMut};
use single_step::Root2;
use std::fmt::format;
use std::fs;

pub struct Memory {
    pub mem: [u8; 65536],
}

impl Memory {
    pub fn new() -> Memory {
        Memory { mem: [0; 65536] }
    }
}

impl Index<u16> for Memory {
    type Output = u8;
    fn index(&self, index: u16) -> &Self::Output {
        &self.mem[index as usize]
    }
}

impl IndexMut<u16> for Memory {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        &mut self.mem[index as usize]
    }
}

#[test]
pub fn run_tests() {
    for i in 0..0x100 {
        use std::env;
        let mut memory: Memory = Memory::new();
        let json_file_path = format!("./65x02/nes6502/v1/{:02x}.json", i);
        println!("{}", json_file_path);
        let file = fs::read(json_file_path).unwrap();
        let tests: Vec<Root2> = serde_json::from_reader(file.as_slice()).unwrap();
        let mut test_number = 1;
        for test in tests {
            let mut cpu_ut = Cpu::new_test(
                test.initial.pc as u16,
                test.initial.s as u8,
                test.initial.a as u8,
                test.initial.x as u8,
                test.initial.y as u8,
                test.initial.p as u8,
            );
            for ram_value in test.initial.ram {
                memory[ram_value[0] as u16] = ram_value[1] as u8;
            }
            cpu_ut.run_instr(&mut memory);

            let cpu_final = Cpu::new_test(
                test.final_field.pc as u16,
                test.final_field.s as u8,
                test.final_field.a as u8,
                test.final_field.x as u8,
                test.final_field.y as u8,
                test.final_field.p as u8,
            );
            for ram_final in test.final_field.ram {
                memory[ram_final[0] as u16] = ram_final[1] as u8;
            }
            //print!("cpu_ut {:#x?}",cpu_ut);
            //print!("cpu_final {:#x?}",cpu_final);
            if cpu_final != cpu_ut {
                println!("instruction {:X} under test", i);
                println!("test number {}", test_number);
                println!("name:{:#x?}", test.name);
                println!("cpu_ut {:#x?}", cpu_ut);
                println!("cpu_final {:#x?}", cpu_final);
                assert_eq!(cpu_final, cpu_ut, "cpu_final != cpu_ut");
                panic!("cpu_final != cpu_ut");
            }
            test_number += 1
        }
    }
}
