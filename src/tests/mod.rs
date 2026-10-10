pub mod single_step;
use crate::Cpu;
use core::ops::{Index, IndexMut};
use single_step::Root2;
use std::fmt::format;
use std::fs;
//use std::slice::range;

#[cfg(not(feature = "decimal"))]
const SUITE_DIR: &str = "./65x02/nes6502/v1";
#[cfg(feature = "decimal")]
const SUITE_DIR: &str = "./65x02/6502/v1";

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
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
        let mut memory_ut: Memory = Memory::new();
        let mut memory_final: Memory = Memory::new();
        let json_file_path = format!("{}/{:02x}.json", SUITE_DIR, i);
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
                memory_ut[ram_value[0] as u16] = ram_value[1] as u8;
            }
            cpu_ut.run_instr(&mut memory_ut);

            let cpu_final = Cpu::new_test(
                test.final_field.pc as u16,
                test.final_field.s as u8,
                test.final_field.a as u8,
                test.final_field.x as u8,
                test.final_field.y as u8,
                test.final_field.p as u8,
            );
            for ram_final in test.final_field.ram {
                memory_final[ram_final[0] as u16] = ram_final[1] as u8;
            }
            if memory_ut != memory_final{
                //println!("Memory ut {:?} != Memory final {:?}", memory_ut, memory_final);
                for ((ut,fin), idx) in (memory_ut.mem.iter().zip(memory_final.mem.iter()).zip(0..0xFFFF)){
                    if(*ut != *fin){
                        println!("Memory ut {:x?} != Memory final {:x?} @ addr {:x?}", *ut, *fin, idx);
                    }
                }
                assert!(false);
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

/// Memory that records every address written through `IndexMut`.
struct TrackedMemory {
    mem: Box<[u8; 65536]>,
    writes: Vec<u16>,
}

impl Index<u16> for TrackedMemory {
    type Output = u8;
    fn index(&self, index: u16) -> &Self::Output {
        &self.mem[index as usize]
    }
}

impl IndexMut<u16> for TrackedMemory {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        self.writes.push(index);
        &mut self.mem[index as usize]
    }
}

/// Runs every opcode of the nes6502 suite without stopping at the first
/// failure and prints a per-opcode summary with the first failing case.
#[test]
pub fn report_all_opcodes() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let only: Option<usize> = std::env::var("OPCODE")
        .ok()
        .map(|s| usize::from_str_radix(&s, 16).unwrap());
    let mut mem = TrackedMemory { mem: Box::new([0; 65536]), writes: Vec::new() };
    let mut failing_opcodes = 0;
    for op in 0..0x100usize {
        if only.map_or(false, |o| o != op) {
            continue;
        }
        let path = format!("{}/{:02x}.json", SUITE_DIR, op);
        let file = fs::read(&path).unwrap();
        let tests: Vec<Root2> = serde_json::from_slice(&file).unwrap();
        let total = tests.len();
        let mut failures = 0;
        let mut first: Option<String> = None;
        for test in tests.iter() {
            for r in &test.initial.ram {
                mem.mem[r[0] as usize] = r[1] as u8;
            }
            mem.writes.clear();
            let mut cpu = Cpu::new_test(
                test.initial.pc as u16,
                test.initial.s as u8,
                test.initial.a as u8,
                test.initial.x as u8,
                test.initial.y as u8,
                test.initial.p as u8,
            );
            let result = catch_unwind(AssertUnwindSafe(|| cpu.run_instr(&mut mem)));
            let f = &test.final_field;
            let mut errs = Vec::new();
            match result {
                Err(e) => {
                    let msg = e
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_default();
                    errs.push(format!("PANIC {}", msg));
                }
                Ok(()) => {
                    let regs = [
                        ("pc", cpu.pc as i64, f.pc),
                        ("s", cpu.sp as i64, f.s),
                        ("a", cpu.a as i64, f.a),
                        ("x", cpu.x as i64, f.x),
                        ("y", cpu.y as i64, f.y),
                        ("p", cpu.s.get() as i64, f.p),
                    ];
                    for (n, got, want) in regs {
                        if got != want {
                            errs.push(format!("{} got {:02x} want {:02x}", n, got, want));
                        }
                    }
                    for r in &f.ram {
                        let got = mem.mem[r[0] as usize];
                        if got as i64 != r[1] {
                            errs.push(format!("mem[{:04x}] got {:02x} want {:02x}", r[0], got, r[1]));
                        }
                    }
                    for w in &mem.writes {
                        if !f.ram.iter().any(|r| r[0] == *w as i64) {
                            errs.push(format!("stray write to {:04x}", w));
                        }
                    }
                    if cpu.cycles != test.cycles.len() as isize {
                        errs.push(format!("cycles got {} want {}", cpu.cycles, test.cycles.len()));
                    }
                }
            }
            if !errs.is_empty() {
                failures += 1;
                if first.is_none() {
                    first = Some(format!(
                        "\"{}\" p={:02x}: {}",
                        test.name,
                        test.initial.p,
                        errs.join(", ")
                    ));
                }
            }
            for r in test.initial.ram.iter().chain(f.ram.iter()) {
                mem.mem[r[0] as usize] = 0;
            }
            for w in mem.writes.drain(..) {
                mem.mem[w as usize] = 0;
            }
        }
        if failures > 0 {
            failing_opcodes += 1;
            println!("{:02X}: {}/{} failed; first {}", op, failures, total, first.unwrap());
        }
    }
    println!("{} opcodes failing", failing_opcodes);
    assert_eq!(failing_opcodes, 0);
}

#[test]
pub fn interrupts_round_trip_through_rti() {
    for (vector, handler) in [(0xFFFAu16, 0x9000u16), (0xFFFE, 0xA000)] {
        let mut mem = Memory::new();
        mem[vector] = handler as u8;
        mem[vector + 1] = (handler >> 8) as u8;
        mem[handler] = 0x40; // RTI
        let mut cpu = Cpu::new_test(0x1234, 0xFD, 0, 0, 0, 0xA1);
        if vector == 0xFFFA {
            cpu.nmi(&mut mem);
        } else {
            cpu.irq(&mut mem);
        }
        assert_eq!(cpu.pc, handler);
        assert_eq!(cpu.sp, 0xFA);
        assert_eq!(mem[0x1FD], 0x12);
        assert_eq!(mem[0x1FC], 0x34);
        assert_eq!(mem[0x1FB], 0xA1, "pushed P must have B clear and bit 5 set");
        assert!(cpu.s.get_interrupt());
        cpu.run_instr(&mut mem);
        assert_eq!((cpu.pc, cpu.sp, cpu.s.get()), (0x1234, 0xFD, 0xA1));
    }
}
