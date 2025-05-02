pub mod cpu;
pub use cpu::Cpu;

#[cfg(test)]
mod test{
    use core::ops::{Index, IndexMut};
    pub struct Memory{
        pub ram: [u8;2048],
        pub rom: [u8;16384],
        pub dummy: u8
    }
    impl Index<u16> for Memory{
        type Output = u8;
        fn index(&self, index:u16) -> &Self::Output {
            match index {
                0x0000..0x0800 => &self.ram[index as usize],
                0x8000..=0xBFFF => &self.rom[index as usize - 0x8000],
                0xC000..=0xFFFF => &self.rom[index as usize - 0xC000],
                    _ => &self.dummy,
            }

        }
    }

    impl IndexMut<u16> for Memory{
        fn index_mut(&mut self, index:u16) -> &mut Self::Output{
            let i = index & 0x3FFF;
            match i {
                0x0000..0x0800 => &mut self.ram[i as usize],
                (0x8000..=0xBFFF) | (0xC000..=0xFFFF) => {
                    #[cfg(feature = "log")]
                    error!("Can't write to ROM");
                    //panic!("can't write to ROM");
                    &mut self.dummy
                },
                _ => &mut self.dummy,
            }
        }
    }
    use super::*;
    use std::{fs, env};
    use std::io::Write;
    use log::log;

    extern crate simple_logger;
    #[test]
    pub fn main(){
        #[cfg(feature = "logging")]
        simple_logger::init_with_level(log::Level::Trace).unwrap();
        let correct_log_path = "correct_log.txt";
        let correct_log_lines:Vec<String>=
            fs::read_to_string(correct_log_path)
                .unwrap()  // panic on possible file-reading errors
                .lines()  // split the string into an iterator of string slices
                .map(String::from)  // make each slice into a string
                .collect();
        let mut log_file = fs::File::create("log.txt").unwrap();
        
        let path = "nestest.nes";
        // let file = fs::read(args[1].clone()).unwrap();
        let file = fs::read(path).unwrap();
        let mut file_array = [0u8;0x4000];
        file_array.copy_from_slice(&file[0x0010..0x4010]);
        let mut mem = Memory{
            ram: [0;0x0800],
            rom: file_array,
            dummy: 0
        };
        let mut core = Cpu::new(None);
        core.start(&mut mem);
        for i in 0..8992{
            for _ in 0..3{
                core.run(&mut mem);
            }
            let log_line = core.log_line.clone();
            log_file.write(log_line.as_bytes()).unwrap();
            println!("{}",log_line);
            for (j, (s,c))in log_line.chars().zip(correct_log_lines[i].chars()).enumerate(){
                if !match j{
                    0..8 => s == c,
                    8..49 => true,
                    49..74 => s == c,
                    74..95=> true,
                    95.. => unreachable!(),
                }{
                    panic!("{}, \n{}", correct_log_lines[i], log_line);
                }
            }
        }
    }
}