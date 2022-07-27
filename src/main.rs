#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use defmt::unwrap;
use defmt_rtt as _;
use embassy_nrf::interrupt::{self, InterruptExt, Priority};
use nrf_modem::{gnss::GnssConfig, ConnectionPreference, SystemMode};
use panic_probe as _;
use futures::StreamExt;

extern crate tinyrlibc;

#[embassy::main]
async fn main(_spawner: embassy::executor::Spawner, _p: embassy_nrf::Peripherals) {
    defmt::println!("Hello, world!");

    // Set up the interrupts for the modem
    let egu1 = embassy_nrf::interrupt::take!(EGU1);
    egu1.set_priority(Priority::P4);
    egu1.set_handler(|_| {
        nrf_modem::application_irq_handler();
        cortex_m::asm::sev();
    });
    egu1.enable();

    let egu2 = embassy_nrf::interrupt::take!(EGU2);
    egu2.set_priority(Priority::P4);
    egu2.set_handler(|_| {
        nrf_modem::trace_irq_handler();
        cortex_m::asm::sev();
    });
    egu2.enable();

    let ipc = embassy_nrf::interrupt::take!(IPC);
    ipc.set_priority(Priority::P0);
    ipc.set_handler(|_| {
        nrf_modem::ipc_irq_handler();
        cortex_m::asm::sev();
    });
    ipc.enable();

    run().await;

    exit();
}

async fn run() {
    // let mut cp = unwrap!(cortex_m::Peripherals::take());
    // let _dp = unwrap!(nrf9160_hal::pac::Peripherals::take());

    defmt::println!("Initializing modem");
    unwrap!(
        nrf_modem::init(SystemMode {
            lte_support: true,
            nbiot_support: true,
            gnss_support: true,
            preference: ConnectionPreference::NetworkPreferenceWithLteFallback,
        })
        .await
    );

    unwrap!(nrf_modem::configure_gnss_on_pca10090ns().await);
    defmt::println!("Initializing gps");
    let mut gnss = unwrap!(nrf_modem::gnss::Gnss::new().await);
    defmt::println!("Starting single fix");
    let mut iter = unwrap!(gnss.start_single_fix(GnssConfig { fix_retry: 300, ..Default::default() }));

    while let Some(x) = iter.next().await {
        defmt::println!("{:?}", defmt::Debug2Format(&x));
    }
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
