#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use core::str::FromStr;
use defmt::unwrap;
use defmt_rtt as _;
use embassy_nrf::interrupt::{Interrupt, InterruptExt, Priority};
use embassy_time::Duration;
use nrf_modem::{no_std_net::SocketAddr, ConnectionPreference, LteLink, SystemMode, TcpStream};

extern crate tinyrlibc;

defmt::timestamp!("{=u64:us}", { embassy_time::Instant::now().as_micros() });

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    defmt::println!("Hello, world!");

    use embassy_nrf::pac::interrupt;

    // Set up the interrupts for the modem
    let egu1 = unsafe { embassy_nrf::interrupt::EGU1::steal() };
    egu1.set_priority(Priority::P4);
    egu1.enable();

    let ipc = unsafe { embassy_nrf::interrupt::IPC::steal() };
    ipc.set_priority(Priority::P0);
    ipc.enable();

    #[cortex_m_rt::interrupt]
    fn EGU1() {
        nrf_modem::application_irq_handler();
        cortex_m::asm::sev();
    }

    #[cortex_m_rt::interrupt]
    fn IPC() {
        nrf_modem::ipc_irq_handler();
        cortex_m::asm::sev();
    }

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
            lte_psm_support: true,
        })
        .await
    );

    // nrf_modem::configure_gnss_on_pca10090ns().await.unwrap();

    // defmt::println!("Initializing gps");
    // let mut gnss = nrf_modem::gnss::Gnss::new().await.unwrap();
    // defmt::println!("Starting single fix");
    // let mut iter = gnss
    //     .start_continuous_fix(nrf_modem::gnss::GnssConfig {
    //         fix_retry: 600,
    //         nmea_mask: nrf_modem::gnss::NmeaMask {
    //             gga: false,
    //             gll: false,
    //             gsa: false,
    //             gsv: true,
    //             rmc: false,
    //         },
    //         ..Default::default()
    //     })
    //     .unwrap();

    // while let Some(x) = futures::StreamExt::next(&mut iter).await {
    //     defmt::println!("{:?}", defmt::Debug2Format(&x));
    // }
    defmt::println!("Creating link");

    let link = LteLink::new().await.unwrap();
    embassy_time::with_timeout(Duration::from_millis(30000), link.wait_for_link())
        .await
        .unwrap()
        .unwrap();

    let google_ip = nrf_modem::get_host_by_name("google.com").await.unwrap();
    defmt::println!("Google ip: {:?}", defmt::Debug2Format(&google_ip));

    let stream = embassy_time::with_timeout(
        Duration::from_millis(2000),
        TcpStream::connect(SocketAddr::from((google_ip, 80))),
    )
    .await
    .unwrap()
    .unwrap();

    stream
        .write("GET / HTTP/1.0\nHost: google.com\r\n\r\n".as_bytes())
        .await
        .unwrap();
    let mut buffer = [0; 1024];
    let used = stream.receive(&mut buffer).await.unwrap();

    defmt::println!("Google page: {}", core::str::from_utf8(used).unwrap());

    let socket = nrf_modem::UdpSocket::bind(SocketAddr::from_str("0.0.0.0:53").unwrap())
        .await
        .unwrap();
    // Do a DNS request
    socket
        .send_to(
            &[
                0xdb, 0x42, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x77,
                0x77, 0x77, 0x0C, 0x6E, 0x6F, 0x72, 0x74, 0x68, 0x65, 0x61, 0x73, 0x74, 0x65, 0x72,
                0x6E, 0x03, 0x65, 0x64, 0x75, 0x00, 0x00, 0x01, 0x00, 0x01,
            ],
            SocketAddr::from_str("8.8.8.8:53").unwrap(),
        )
        .await
        .unwrap();
    let (result, source) = socket.receive_from(&mut buffer).await.unwrap();

    defmt::println!("Result: {:X}", result);
    defmt::println!("Source: {}", defmt::Debug2Format(&source));
}

/// Called when our code panics.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    cortex_m::interrupt::disable();

    defmt::println!("Panic: {}", defmt::Display2Format(info));

    // Make this a hardfault. This has a lot of advantages:
    // - No interrupt can interrupt a hardfault
    // - Recursion cannot happen because the hardware prevents that
    // - It is the natural endpoint for program failures on cortex-m
    cortex_m::asm::udf();
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
