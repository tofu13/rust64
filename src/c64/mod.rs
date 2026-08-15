// main module for C64 updates
extern crate minifb;

pub mod cpu;
pub mod crt;
pub mod memory;
pub mod opcodes;
pub mod vic;

mod cia;
mod clock;
mod io;
mod sid;
mod sid_tables;
mod vic_tables;

use debugger;
use log::info;
use minifb::*;
use std::collections::VecDeque;
use utils;

pub const SCREEN_WIDTH: usize = 384; // extend 20 pixels left and right for the borders
pub const SCREEN_HEIGHT: usize = 272; // extend 36 pixels top and down for the borders

// PAL clock frequency in Hz
const CLOCK_FREQ: f64 = 985248.0;
pub const RATIO_MOVING_AVERAGE_COUNT: usize = 10; // Number of values for calculating average ratio

pub struct C64 {
    pub main_window: Window,
    pub file_to_load: String,
    pub crt_to_load: String,
    memory: memory::MemShared,
    io: io::IO,
    clock: clock::Clock,
    cpu: cpu::CPUShared,
    cia1: cia::CIAShared,
    cia2: cia::CIAShared,
    vic: vic::VICShared,
    sid: sid::SIDShared,

    debugger: Option<debugger::Debugger>,
    powered_on: bool,
    boot_complete: bool,
    cycle_count: u32,

    mute: bool,
    sample_count: u32,
    ratio_buffer: VecDeque<f64>,
    frame_count: u32,
}

impl C64 {
    pub fn new(
        window_scale: Scale,
        debugger_on: bool,
        prg_to_load: &str,
        crt_to_load: &str,
        mute: bool,
        warp: bool,
    ) -> C64 {
        let memory = memory::Memory::new_shared();
        let vic = vic::VIC::new_shared();
        let cia1 = cia::CIA::new_shared(true);
        let cia2 = cia::CIA::new_shared(false);
        let cpu = cpu::CPU::new_shared();
        let sid = sid::SID::new_shared();

        let mut c64 = C64 {
            main_window: Window::new(
                "Rust64",
                SCREEN_WIDTH,
                SCREEN_HEIGHT,
                WindowOptions {
                    scale: window_scale,
                    ..Default::default()
                },
            )
            .unwrap(),
            file_to_load: String::from(prg_to_load),
            crt_to_load: String::from(crt_to_load),
            memory: memory.clone(), // shared system memory (RAM, ROM, IO registers)
            io: io::IO::new(),
            clock: clock::Clock::new(),
            cpu: cpu.clone(),
            cia1: cia1.clone(),
            cia2: cia2.clone(),
            vic: vic.clone(),
            sid: sid.clone(),
            debugger: if debugger_on {
                Some(debugger::Debugger::new())
            } else {
                None
            },
            powered_on: false,
            boot_complete: false,
            cycle_count: 0,
            mute,
            sample_count: 0,
            ratio_buffer: VecDeque::new(),
            frame_count: 0,
        };

        c64.main_window.set_position(75, 20);
        c64.main_window.set_target_fps(if warp { 0 } else { 50 });

        // cyclic dependencies are not possible in Rust (yet?), so we have
        // to resort to setting references manually
        c64.cia1
            .borrow_mut()
            .set_references(memory.clone(), cpu.clone(), vic.clone());
        c64.cia2
            .borrow_mut()
            .set_references(memory.clone(), cpu.clone(), vic.clone());
        c64.vic
            .borrow_mut()
            .set_references(memory.clone(), cpu.clone());
        c64.sid.borrow_mut().set_references(memory.clone());
        c64.cpu.borrow_mut().set_references(
            memory.clone(),
            vic.clone(),
            cia1.clone(),
            cia2.clone(),
            sid.clone(),
        );

        drop(memory);
        drop(cia1);
        drop(cia2);
        drop(vic);
        drop(cpu);
        drop(sid);

        c64
    }

    pub fn reset(&mut self) {
        self.memory.borrow_mut().reset();
        self.cpu.borrow_mut().reset();
        self.cia1.borrow_mut().reset();
        self.cia2.borrow_mut().reset();
        self.sid.borrow_mut().reset();
    }

    pub fn run(&mut self) {
        // attempt to load a program supplied with command line
        if !self.powered_on {
            // $FCE2 is the power-on reset routine, which searches for and starts
            // a cartridge amongst other things. The cartridge must be loaded here
            self.powered_on = self.cpu.borrow_mut().pc == 0xFCE2;
            if self.powered_on {
                let crt_file = &self.crt_to_load.to_owned()[..];
                if !crt_file.is_empty() {
                    let crt = crt::Crt::from_filename(crt_file).unwrap();
                    info!("Loading crt {:?}", crt);
                    crt.load_into_memory(self.memory.borrow_mut());
                }
            }
        }

        if !self.boot_complete {
            // $A480 is the BASIC warm start sequence - safe to assume we can load a cmdline program now
            self.boot_complete = self.cpu.borrow_mut().pc == 0xA480;

            if self.boot_complete {
                let prg_file = &self.file_to_load.to_owned()[..];

                if !prg_file.is_empty() {
                    self.boot_complete = true;
                    self.load_prg(prg_file);
                }
            }
        }

        // main C64 update
        let mut should_trigger_vblank = false;

        if self
            .vic
            .borrow_mut()
            .update(self.cycle_count, &mut should_trigger_vblank)
        {
            self.sid.borrow_mut().update();
        }

        self.cia1.borrow_mut().process_irq();
        self.cia2.borrow_mut().process_irq();
        self.cia1.borrow_mut().update();
        self.cia2.borrow_mut().update();

        self.cpu.borrow_mut().update(self.cycle_count);

        // update the debugger window if it exists
        if let Some(ref mut dbg) = self.debugger {
            dbg.update_vic_window(&mut self.vic);
            if should_trigger_vblank {
                dbg.render(&mut self.cpu, &mut self.memory);
            }
        }

        // redraw the screen and process input on VBlank
        if should_trigger_vblank {
            let _ = self.main_window.update_with_buffer(
                &self.vic.borrow_mut().window_buffer,
                SCREEN_WIDTH,
                SCREEN_HEIGHT,
            );
            self.frame_count += 1;
            self.io.update(&self.main_window, &mut self.cia1);
            self.cia1.borrow_mut().count_tod();
            self.cia2.borrow_mut().count_tod();

            // region maintenance tasks

            // handle RESTORE key
            if self.io.check_restore_key(&self.main_window) {
                self.cpu.borrow_mut().set_nmi(true);
            }
            // process F11 for console ASM
            if self.main_window.is_key_pressed(Key::F11, KeyRepeat::No) {
                let di = self.cpu.borrow_mut().debug_instr;
                self.cpu.borrow_mut().debug_instr = !di;
            }
            // process F12 for reset
            if self.main_window.is_key_pressed(Key::F12, KeyRepeat::No) {
                self.reset();
            }
            // get clock ratios and FPS
            if let Some(elapsed) = self.clock.sample() {
                let clock_ratio = self.clock_ratio(elapsed); // Future usage for throttling
                let fps = self.frame_count as f64 / elapsed;
                self.frame_count = 0;
                info!(
                    "{:.6} second passed - CPU current {:.2}%, CPU average {:.0}%, {:.1} FPS",
                    elapsed,
                    clock_ratio * 100.0,
                    self.average_clock_ratio() * 100.0,
                    fps,
                );
            }
            //endregion
        }

        self.cycle_count += 1;

        // update SDL2 audio buffers
        if !self.mute {
            self.sid.borrow_mut().update_audio();
        }
    }

    // *** private functions *** //

    fn clock_ratio(&mut self, elapsed: f64) -> f64 {
        // Returns a measure of simulation speed vs standard clock
        let ratio = (self.cycle_count - self.sample_count) as f64 / elapsed / CLOCK_FREQ;
        self.sample_count = self.cycle_count;

        // Store data for average
        self.ratio_buffer.push_front(ratio);
        // Evict oldest sample
        self.ratio_buffer.truncate(RATIO_MOVING_AVERAGE_COUNT);

        ratio
    }

    fn average_clock_ratio(&self) -> f64 {
        // Returns an average measure of simulation speed vs standard clock
        self.ratio_buffer.iter().sum::<f64>() / (self.ratio_buffer.len() as f64)
    }

    // load a *.prg file
    fn load_prg(&mut self, filename: &str) {
        let prg_data = utils::open_file(filename, 0);
        let start_address: u16 = ((prg_data[1] as u16) << 8) | (prg_data[0] as u16);
        info!(
            "Loading {} to start location at ${:04x} ({})",
            filename, start_address, start_address
        );

        for (i, data) in prg_data.iter().enumerate().skip(2) {
            self.memory
                .borrow_mut()
                .write_byte(start_address + (i as u16) - 2, *data);
        }
    }
}
