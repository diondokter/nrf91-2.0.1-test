#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use core::str::FromStr;
use defmt::unwrap;
use defmt_rtt as _;
use embassy::time::Duration;
use embassy_nrf::interrupt::{self, InterruptExt, Priority};
use nrf_modem::{
    lte_link::LteLink, no_std_net::SocketAddr, tcp_stream::TcpStream, ConnectionPreference,
    SystemMode,
};
use panic_probe as _;

extern crate tinyrlibc;

defmt::timestamp!("{=u64:us}", { embassy::time::Instant::now().as_micros() });

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
    defmt::println!("Creating link");

    let link = LteLink::new().await.unwrap();
    embassy::time::with_timeout(Duration::from_millis(30000), link.wait_for_link())
        .await
        .unwrap()
        .unwrap();
    let google_ip = nrf_modem::dns::get_host_by_name("google.com").unwrap();
    defmt::println!("Google ip: {:?}", defmt::Debug2Format(&google_ip));
    let stream = embassy::time::with_timeout(
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

    drop(stream);

    let socket =
        nrf_modem::udp_socket::UdpSocket::bind(SocketAddr::from_str("0.0.0.0:53").unwrap())
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

    drop(socket);

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
