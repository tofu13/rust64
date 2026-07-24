extern crate byteorder;
extern crate clap;
extern crate minifb;
extern crate num;

#[macro_use]
extern crate enum_primitive;
extern crate fern;
extern crate log;

#[macro_use]
mod utils;
mod c64;
mod debugger;

use clap::Parser;
use log::{info, LevelFilter};
use minifb::Scale;

#[derive(Parser)]
#[command(about = "rust64 - C64 emulator")]
struct Args {
    /// Program or cartridge file to load (.prg / .crt)
    file: Option<String>,

    /// Scale window 2x
    #[arg(long)]
    x2: bool,

    /// Enable debugger
    #[arg(short, long)]
    debugger: bool,

    /// Disable sound
    #[arg[short, long]]
    mute: bool,
}

#[cfg(debug_assertions)]
fn log_output() -> fern::Output {
    std::io::stdout().into()
}

#[cfg(not(debug_assertions))]
fn log_output() -> fern::Output {
    fern::log_file("rust64.log")
        .expect("failed to open log file")
        .into()
}

fn setup_logging() {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {}] {}",
                record.level(),
                record.target(),
                message
            ))
        })
        .level(LevelFilter::Info)
        .chain(log_output())
        .apply()
        .expect("failed to initialize logger");
}

fn main() {
    setup_logging();
    info!("Rust64 starting");

    let args = Args::parse();

    let file = args.file.as_deref().unwrap_or("");
    let prg_to_load = if file.ends_with(".prg") { file } else { "" };
    let crt_to_load = if file.ends_with(".crt") { file } else { "" };
    let window_scale = if args.x2 { Scale::X2 } else { Scale::X1 };

    let mut c64 = c64::C64::new(window_scale, args.debugger, prg_to_load, crt_to_load);
    let mut c64 = c64::C64::new(
        window_scale,
        args.debugger,
        prg_to_load,
        crt_to_load,
        args.mute,
    );
    c64.reset();

    // main update loop
    while c64.main_window.is_open() {
        c64.run();
    }
    info!("Rust64 ending");
}
