use embassy_rp::Peripheral;
use embassy_rp::{
    i2c::{self, Config as I2CConfig, SdaPin, SclPin, Instance},
};
use embedded_graphics::{
    mono_font::{
        ascii::{FONT_10X20, FONT_6X9, FONT_7X14},
        MonoTextStyleBuilder,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, StrokeAlignment},
    text::{Alignment, Text},
};
use embassy_net::{StaticConfigV4, HardwareAddress};
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use heapless::String;
use core::fmt;

pub struct Display<'d, T>(Ssd1306<I2CInterface<embassy_rp::i2c::I2c<'d, T, embassy_rp::i2c::Blocking>>, ssd1306::size::DisplaySize128x64, ssd1306::mode::BufferedGraphicsMode<ssd1306::size::DisplaySize128x64>>) where T: Instance;

impl <'d, T>Display<'d, T> where T: Instance {
    pub fn new(peri: impl Peripheral<P = T> + 'd, scl: impl Peripheral<P = impl SclPin<T>> + 'd, sda: impl Peripheral<P = impl SdaPin<T>> + 'd) -> Self {
        let i2c = i2c::I2c::new_blocking(peri, scl, sda, I2CConfig::default());
        let interface = I2CDisplayInterface::new(i2c);
        let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0).into_buffered_graphics_mode();
        display.init().unwrap();
        let _ = display.clear(BinaryColor::Off);

        let border_stroke = PrimitiveStyleBuilder::new()
            .stroke_color(BinaryColor::On)
            .stroke_width(1)
            .stroke_alignment(StrokeAlignment::Inside)
            .build();

        display
            .bounding_box()
            .into_styled(border_stroke)
            .draw(&mut display)
            .unwrap();

        Self(display)
    }

    pub fn show_configuration(&mut self, ip: &StaticConfigV4, hardware: &HardwareAddress) {
        let _ = self.0.clear(BinaryColor::Off);
        let medium_font = MonoTextStyleBuilder::new()
            .font(&FONT_7X14)
            .text_color(BinaryColor::On)
            .background_color(BinaryColor::Off)
            .build();

        let mut formatted_ip: String<20> = String::new();
        fmt::write(&mut formatted_ip, format_args!("{}", ip.address)).unwrap();
        Text::with_alignment(&formatted_ip, Point::new(64, 20), medium_font, Alignment::Center)
            .draw(&mut self.0)
            .unwrap();

        let mut formatted_hardware: String<20> = String::new();
        fmt::write(&mut formatted_hardware, format_args!("{}", hardware)).unwrap();
        Text::with_alignment(
            &formatted_hardware,
            Point::new(64, 40),
            medium_font,
            Alignment::Center,
        )
        .draw(&mut self.0)
        .unwrap();
        self.0.flush().unwrap();
    }

    fn show_measurement(&mut self, measurement: Option<f32>, position: Point) {
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_10X20)
            .text_color(BinaryColor::On)
            .background_color(BinaryColor::Off)
            .build();

        match measurement {
            Some(temp) => {
                let mut formatted_temp: String<7> = String::new();
                fmt::write(&mut formatted_temp, format_args!("{:.1}", temp)).unwrap();
                Text::with_alignment(
                    &formatted_temp,
                    position,
                    text_style,
                    Alignment::Center,
                )
                .draw(&mut self.0)
                .unwrap();
            },
            None => {
                Text::with_alignment("--", position, text_style, Alignment::Center)
                    .draw(&mut self.0)
                    .unwrap();
            }
        }
    }

    pub fn show_measurements(&mut self, a: Option<f32>, b: Option<f32>, c: Option<f32>, d: Option<f32>) {
        let _ = self.0.clear(BinaryColor::Off);
        let small_font = MonoTextStyleBuilder::new()
            .font(&FONT_6X9)
            .text_color(BinaryColor::On)
            .background_color(BinaryColor::Off)
            .build();

        Text::with_alignment("A B\nC D", Point::new(64, 28), small_font, Alignment::Center)
            .draw(&mut self.0)
            .unwrap();

        self.show_measurement(a, Point::new(32, 20));
        self.show_measurement(b, Point::new(96, 20));
        self.show_measurement(c, Point::new(32, 54));
        self.show_measurement(d, Point::new(96, 54));
        self.0.flush().unwrap();
    }
}
