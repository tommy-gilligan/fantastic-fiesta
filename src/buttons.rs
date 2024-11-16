use defmt::*;
use embassy_futures::select::{select3, Either3};
use embassy_rp::{
    gpio::{Input, Pin, Pull},
    Peripheral,
};
use {defmt_rtt as _, panic_probe as _};

#[derive(Debug, Format)]
pub enum ButtonPress {
    Up,
    Select,
    Down,
}

pub struct Buttons<'a> {
    up: Input<'a>,
    select: Input<'a>,
    down: Input<'a>,
}

impl<'a> Buttons<'a> {
    pub fn new(
        up: impl Peripheral<P = impl Pin> + 'a,
        select: impl Peripheral<P = impl Pin> + 'a,
        down: impl Peripheral<P = impl Pin> + 'a,
    ) -> Self {
        Self {
            up: Input::new(up, Pull::Up),
            select: Input::new(select, Pull::Up),
            down: Input::new(down, Pull::Up),
        }
    }

    pub async fn pressed(&mut self) -> ButtonPress {
        match select3(
            self.up.wait_for_rising_edge(),
            self.select.wait_for_rising_edge(),
            self.down.wait_for_rising_edge()
        ).await {
            Either3::First(_) => ButtonPress::Up,
            Either3::Second(_) => ButtonPress::Select,
            Either3::Third(_) => ButtonPress::Down,
        }
    }
}
