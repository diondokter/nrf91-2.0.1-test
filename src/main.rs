#![no_main]
#![no_std]

use core::str::FromStr;
use cortex_m_rt::ExceptionFrame;
use defmt::unwrap;
use defmt_rtt as _;
use embassy_nrf::interrupt;
use embassy_time::Duration;
use nrf_modem::{no_std_net::SocketAddr, ConnectionPreference, LteLink, SystemMode, TcpStream};
use panic_probe as _;

extern crate tinyrlibc;

defmt::timestamp!("{=u64:us}", { embassy_time::Instant::now().as_micros() });

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    defmt::println!("Hello, world!");

    // Interrupt Handler for LTE related hardware. Defer straight to the library.
    #[cortex_m_rt::interrupt]
    #[allow(non_snake_case)]
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

    nrf_modem::configure_gnss_on_pca10090ns().await.unwrap();

    // defmt::println!("Initializing gps");
    // let gnss = nrf_modem::Gnss::new().await.unwrap();
    // defmt::println!("Starting single fix");
    // let mut iter = gnss
    //     .start_continuous_fix(nrf_modem::GnssConfig {
    //         nmea_mask: nrf_modem::NmeaMask {
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
    //     break;
    // }

    // iter.deactivate().await.unwrap();

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

    stream.deactivate().await.unwrap();

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

    socket.deactivate().await.unwrap();
    link.deactivate().await.unwrap();

    defmt::println!("Result: {:X}", result);
    defmt::println!("Source: {}", defmt::Debug2Format(&source));
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

#[cortex_m_rt::exception]
unsafe fn HardFault(e: &ExceptionFrame) -> ! {
    defmt::error!("HardFault: {}", defmt::Debug2Format(e));

    loop {
        cortex_m::asm::bkpt();
    }
}

#[link_section = ".spm"]
#[used]
static SPM: [u8; 24052] = *include_bytes!("zephyr.bin");
