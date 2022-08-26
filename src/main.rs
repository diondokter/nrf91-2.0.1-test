#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use defmt::unwrap;
use defmt_rtt as _;
use embassy_nrf::interrupt::{self, InterruptExt, Priority};
use nrf_modem::lte_link::LteLink;
use nrf_modem::no_std_net::SocketAddr;
use nrf_modem::{
    ConnectionPreference, SystemMode,
};
use panic_probe as _;

extern crate tinyrlibc;

defmt::timestamp!("{=u64:us}", {
    embassy::time::Instant::now().as_micros()
});

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

    let regulators: embassy_nrf::pac::REGULATORS = unsafe { core::mem::transmute(()) };
    regulators.dcdcen.modify(|_, w| w.dcdcen().enabled());

    run().await;

    exit();
}

async fn run() {
    defmt::println!("Initializing modem");
    unwrap!(
        nrf_modem::init(SystemMode {
            lte_support: true,
            nbiot_support: false,
            gnss_support: true,
            preference: ConnectionPreference::Lte,
        })
        .await
    );

    // nrf_modem::configure_gnss_on_pca10090ns().await.unwrap();

    // defmt::println!("Initializing gps");
    // let mut gnss = nrf_modem::gnss::Gnss::new().await.unwrap();
    // defmt::println!("Starting single fix");
    // let mut iter = gnss
    //     .start_continuous_fix(GnssConfig {
    //         fix_retry: 600,
    //         nmea_mask: NmeaMask {
    //             gga: false,
    //             gll: false,
    //             gsa: false,
    //             gsv: false,
    //             rmc: false,
    //         },
    //         ..Default::default()
    //     })
    //     .unwrap();

    // while let Some(x) = iter.next().await {
    //     defmt::println!("{:?}", defmt::Debug2Format(&x));
    // }

    let link = LteLink::new().await.unwrap();
    link.wait_for_link().await.unwrap();
    let google_ip = nrf_modem::dns::get_host_by_name("google.com").unwrap();
    defmt::println!("Google ip: {:?}", defmt::Debug2Format(&google_ip));
    let stream = nrf_modem::tcp_stream::TcpStream::connect(SocketAddr::from((google_ip, 80)))
        .await
        .unwrap();

    stream.send("GET / HTTP/1.0\nHost: google.com\r\n\r\n".as_bytes()).await.unwrap();
    let mut buffer = [0; 1024];
    let used = stream.receive(&mut buffer).await.unwrap();

    defmt::println!("Google page: {}", core::str::from_utf8(used).unwrap());

    exit();
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
