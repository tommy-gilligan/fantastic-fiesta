#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

mod buttons;
mod sensor;
mod status_leds;
mod display;
mod network;

use assign_resources::assign_resources;
use cyw43::JoinOptions;
use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::{Executor, Spawner};
use embassy_futures::select::{select, Either};
use embassy_net::{tcp::TcpSocket, Config, StackResources};
use embassy_rp::{
    bind_interrupts,
    clocks::RoscRng,
    gpio::{Level, Output},
    multicore::{spawn_core1, Stack},
    peripherals,
    peripherals::{DMA_CH0, PIO0, PIO1},
    pio::{InterruptHandler, Pio},
    pio_programs::onewire::{PioOneWireProgram, PioOneWire}
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, pubsub::PubSubChannel};
use embassy_time::{Duration, Timer};
use heapless::String;
use rand::RngCore;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use buttons::{Buttons, ButtonPress};
use sensor::Ds18b20;
use status_leds::StatusLeds;
use display::Display;

enum ConfigurationState {
    WifiUp {
        ip: embassy_net::StaticConfigV4,
        hardware: embassy_net::HardwareAddress,
    },
    WifiDown,
}

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

static MEASUREMENT_CHANNEL: PubSubChannel<CriticalSectionRawMutex, Option<f32>, 4, 4, 4> = PubSubChannel::new();
static CONFIGURATION_CHANNEL: Channel<CriticalSectionRawMutex, ConfigurationState, 1> = Channel::new();

assign_resources! {
    network: Network {
        pin_23: PIN_23,
        pin_25: PIN_25,
        pin_24: PIN_24,
        pin_29: PIN_29,
        dma_ch0: DMA_CH0,
        pio_0: PIO0,
    },
    user_interface: UserInterface {
        pin_13: PIN_13,
        pin_14: PIN_14,
        pin_15: PIN_15,
        pin_16: PIN_16,
        pin_17: PIN_17,
        pin_18: PIN_18,
        pin_4: PIN_4,
        pin_5: PIN_5,
        i2c_0: I2C0,
    },
    measurement: Measurement {
        pin_12: PIN_12,
        pin_11: PIN_11,
        pin_9: PIN_9,
        pin_6: PIN_6,
        pio_1: PIO1,
    },
}

#[derive(Debug, Format)]
struct Configuration {
    ssid: String<32>,
    password: String<63>,
}

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    PIO1_IRQ_0 => InterruptHandler<PIO1>;
});

#[embassy_executor::task]
async fn cyw43_task(runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn network(spawner: Spawner, r: Network) {
    let c = Configuration {
        ssid: String::try_from("").unwrap(),
        password: String::try_from("").unwrap(),
    };
    info!("{:?}", &c);

    let mut rng = RoscRng;
    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

    // To make flashing faster for development, you may want to flash the firmwares independently
    // at hardcoded addresses, instead of baking them into the program with `include_bytes!`:
    //     probe-rs download 43439A0.bin --binary-format bin --chip RP2040 --base-address 0x10100000
    //     probe-rs download 43439A0_clm.bin --binary-format bin --chip RP2040 --base-address 0x10140000
    //let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 230321) };
    //let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };
    let pwr = Output::new(r.pin_23, Level::Low);
    let cs = Output::new(r.pin_25, Level::High);
    let mut pio = Pio::new(r.pio_0, Irqs);
    let spi = PioSpi::new(&mut pio.common, pio.sm0, pio.irq0, cs, r.pin_24, r.pin_29, r.dma_ch0);

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(cyw43_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let config = Config::dhcpv4(Default::default());

    let seed = rng.next_u64();
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(net_device, config, RESOURCES.init(StackResources::new()), seed);
    unwrap!(spawner.spawn(net_task(runner)));

    loop {
        match control.join(&c.ssid, JoinOptions::new(c.password.as_bytes())).await {
            Ok(_) => break,
            Err(err) => {
                info!("join failed with status={}", err.status);
            }
        }
    }

    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }

    if stack.is_config_up() && stack.is_link_up() {
        CONFIGURATION_CHANNEL
            .send(ConfigurationState::WifiUp {
                ip: stack.config_v4().unwrap(),
                hardware: stack.hardware_address(),
            })
            .await;
        let mut subscriber = MEASUREMENT_CHANNEL.subscriber().unwrap();

        network::listen(stack, &mut subscriber).await;
    } else {
        CONFIGURATION_CHANNEL.send(ConfigurationState::WifiDown).await;
    }
}

#[embassy_executor::task]
pub async fn core1_task(spawner: Spawner, ui: UserInterface, m: Measurement) {
    spawner.spawn(user_interface(spawner, ui)).unwrap();
    spawner.spawn(measurement(spawner, m)).unwrap();
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());
    let r = split_resources! {p};

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| unwrap!(spawner.spawn(core1_task(spawner, r.user_interface, r.measurement))));
        },
    );

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| unwrap!(spawner.spawn(network(spawner, r.network))));
}

#[embassy_executor::task]
pub async fn measurement(_spawner: Spawner, r: Measurement) {
    let publisher = MEASUREMENT_CHANNEL.publisher().unwrap();

    let mut pio = Pio::new(r.pio_1, Irqs);
    let prg = PioOneWireProgram::new(&mut pio.common);
    let mut sensor = Ds18b20::new(PioOneWire::new(&mut pio.common, pio.sm0, r.pin_12, &prg));

    loop {
        sensor.start().await;
        Timer::after_secs(1).await;
        publisher.publish_immediate(sensor.temperature().await.ok());
        Timer::after_millis(1000).await;
    }
}

#[embassy_executor::task]
pub async fn user_interface(_spawner: Spawner, r: UserInterface) {
    let mut status_leds = StatusLeds::new(r.pin_13, r.pin_14, r.pin_15);
    status_leds.turn_on_power();
    let mut buttons = Buttons::new(r.pin_18, r.pin_17, r.pin_16);
    let mut measurements = MEASUREMENT_CHANNEL.subscriber().unwrap();
    let mut display = Display::new(r.i2c_0, r.pin_5, r.pin_4);

    loop {
        Timer::after_millis(1000).await;
        display.show_measurements(
            None,
            measurements.next_message_pure().await,
            None,
            None,
        );
    }

    if let ConfigurationState::WifiUp { ip, hardware } = CONFIGURATION_CHANNEL.receive().await {
        status_leds.turn_on_wifi();
    //     let mut showing_configuration = false;
    //     let mut measurement_state = MeasurementState {
    //         a: None,
    //         b: None,
    //         c: None,
    //         d: None,
    //     };
    //     display.show_measurements(
    //         measurement_state.a,
    //         measurement_state.b,
    //         measurement_state.c,
    //         measurement_state.d,
    //     );

    //     loop {
    //         match select(buttons.pressed(), measurements.next_message_pure()).await {
    //             Either::First(button_press) => {
    //                 println!("{:?}", button_press);
    //                 match button_press {
    //                     ButtonPress::Select => {
    //                         if showing_configuration {
    //                             showing_configuration = false;
    //                             display.show_measurements(
    //                                 measurement_state.a,
    //                                 measurement_state.b,
    //                                 measurement_state.c,
    //                                 measurement_state.d,
    //                             );
    //                         } else {
    //                             showing_configuration = true;
    //                             display.show_configuration(&ip, &hardware);
    //                         }
    //                     }
    //                     _ => {}
    //                 }
    //             }
    //             Either::Second(measurement) => {
    //                 measurement_state = measurement;
    //                 if !showing_configuration {
    //                     display.show_measurements(
    //                         measurement_state.a,
    //                         measurement_state.b,
    //                         measurement_state.c,
    //                         measurement_state.d,
    //                     );
    //                 }
    //             }
    //         }
    //     }
    };
}
