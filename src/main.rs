#![no_main]
#![no_std]
#![feature(async_iterator)]

use defmt_rtt as _; // global logger
use nrf9160_hal::{
    pac::{self, interrupt},
};
use nrf_modem::gnss::GnssConfig;
use panic_probe as _;
use defmt::unwrap;

extern crate tinyrlibc;

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Hello, world!");

    run();

    exit();
}

fn run() {
    let mut cp = unwrap!(cortex_m::Peripherals::take());
    let dp = unwrap!(nrf9160_hal::pac::Peripherals::take());

    // Enable the modem interrupts
    unsafe {
        nrf9160_hal::pac::NVIC::unmask(nrf9160_hal::pac::Interrupt::EGU1);
        nrf9160_hal::pac::NVIC::unmask(nrf9160_hal::pac::Interrupt::EGU2);
        nrf9160_hal::pac::NVIC::unmask(nrf9160_hal::pac::Interrupt::IPC);
        cp.NVIC.set_priority(nrf9160_hal::pac::Interrupt::EGU1, 6 << 5);
        cp.NVIC.set_priority(nrf9160_hal::pac::Interrupt::EGU2, 6 << 5);
        cp.NVIC.set_priority(nrf9160_hal::pac::Interrupt::IPC, 0 << 5);
    }

    nrf_modem::init().unwrap();

    let mut gnss = nrf_modem::gnss::Gnss::new().unwrap();
    let iter = gnss.start_single_fix(GnssConfig::default()).unwrap();
}


// same panicking *behavior* as `panic-probe` but doesn't print a panic message
// this prevents the panic message being printed *twice* when `defmt::panic` is invoked
#[defmt::panic_handler]
fn panic() -> ! {
    cortex_m::asm::udf()
}

/// Terminates the application and makes `probe-run` exit with exit-code = 0
pub fn exit() -> ! {
    loop {
        cortex_m::asm::bkpt();
    }
}

#[link_section = ".spm"]
#[used]
static SPM: [u8; 24052] = *include_bytes!("zephyr.bin");

/// Interrupt Handler for LTE related hardware. Defer straight to the library.
#[interrupt]
fn EGU1() {
    nrf_modem::application_irq_handler();
    cortex_m::asm::sev();
}

/// Interrupt Handler for LTE related hardware. Defer straight to the library.
#[interrupt]
fn EGU2() {
    nrf_modem::trace_irq_handler();
    cortex_m::asm::sev();
}

/// Interrupt Handler for LTE related hardware. Defer straight to the library.
#[interrupt]
fn IPC() {
    nrf_modem::ipc_irq_handler();
    cortex_m::asm::sev();
}