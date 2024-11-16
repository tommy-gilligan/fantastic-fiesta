use embassy_rp::{
    gpio::{Output, Pin, Level},
    Peripheral,
};

pub struct StatusLeds<'a> {
    power: Output<'a>,
    wifi: Output<'a>,
    #[expect(dead_code)]
    ap: Output<'a>,
}

impl <'a>StatusLeds<'a> {
    pub fn new(power: impl Peripheral<P = impl Pin> + 'a, wifi: impl Peripheral<P = impl Pin> + 'a, ap: impl Peripheral<P = impl Pin> + 'a) -> Self {
        Self {
            power: Output::new(power, Level::Low),
            wifi: Output::new(wifi, Level::Low),
            ap: Output::new(ap, Level::Low),
        }
    }

    pub fn turn_on_power(&mut self) {
        self.power.set_high();
    }

    pub fn turn_on_wifi(&mut self) {
        self.wifi.set_high();
    }
}
