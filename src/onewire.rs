pub mod rom;
pub mod scratchpad;

use embassy_rp::pio::{
    self, instr, Common, Config, Instance, LoadedProgram, ShiftConfig, ShiftDirection, StateMachine,
};

pub struct Onewire<'a, PIO: Instance, const SM: usize> {
    sm: StateMachine<'a, PIO, SM>,
    prg: LoadedProgram<'a, PIO>,
    onewire_offset_reset_bus: i32,
    onewire_offset_fetch_bit: i32,
    offset: u8,
    common: Common<'a, PIO>,
    pin: &'a [&'a pio::Pin<'a, PIO>],
}

impl<'a, PIO: Instance, const SM: usize> Onewire<'a, PIO, SM> {
    pub fn new(mut common: Common<'a, PIO>, sm: StateMachine<'a, PIO, SM>, pin: &'a [&'a pio::Pin<'a, PIO>]) -> Self {
        let prg = pio_proc::pio_file!(
            "./src/onewire.pio",
            select_program("onewire"),
            options(max_program_size = 32)
        );

        let mut result = Self {
            prg: common.load_program(&prg.program),
            offset: prg.program.origin.unwrap_or(0),
            onewire_offset_reset_bus: prg.public_defines.reset_bus,
            onewire_offset_fetch_bit: prg.public_defines.fetch_bit,
            common,
            sm,
            pin,
        };
        result.init(result.onewire_offset_fetch_bit, 8);
        result
    }

    pub fn init(&mut self, address: i32, bits_per_word: u8) {
        self.sm.set_config(&config(&self.prg, self.pin, bits_per_word));
        self.jump(address.try_into().unwrap());
        self.sm.set_enable(true);
    }

    pub async fn send(&mut self, data: u32) {
        self.sm.tx().wait_push(data).await;
        self.sm.rx().wait_pull().await;
    }

    pub async fn read(&mut self) -> u8 {
        self.sm.tx().wait_push(0xff).await;
        (self.sm.rx().wait_pull().await >> 24) as u8
    }

    pub async fn romsearch(
        &mut self,
        devices: &mut [Option<rom::RegistrationNumber>],
        command: u32,
    ) -> Result<usize, rom::SearchErr> {
        let mut num_found: usize = 0;
        let mut finished: bool = false;
        let mut branch_point: isize = 0;
        let mut next_branch_point: isize = -1;
        let maxdevs = devices.len();
        let mut romcode: u64 = 0;

        self.init(self.offset.into(), 1);

        while !finished && (maxdevs == 0 || num_found < maxdevs) {
            finished = true;
            branch_point = next_branch_point;
            if !(self.reset().await) {
                self.init(self.offset.into(), 8);
                return Err(rom::SearchErr::NoReply);
            }

            // send search command as single bits
            for i in 0..8 {
                self.send(command >> i).await;
            }

            for index in 0..64 {
                let a = self.read().await;
                let b = self.read().await;
                if a == 0 && b == 0 {
                    if index == branch_point {
                        self.send(1).await;
                        romcode |= 1 << index;
                    } else if index > branch_point || (romcode & (1 << index)) == 0 {
                        self.send(0).await;
                        finished = false;
                        romcode &= !(1 << index);
                        next_branch_point = index;
                    // index < branch_point or romcode[index] = 1
                    } else {
                        self.send(1).await;
                    }
                } else if a != 0 && b != 0 {
                    self.init(self.offset.into(), 8);
                    return Err(rom::SearchErr::DisconnectDuringSearch);
                } else {
                    // (a, b) = (0, 1) or (1, 0)
                    if a == 0 {
                        self.send(0).await;
                        romcode &= !(1 << index);
                    } else {
                        self.send(1).await;
                        romcode |= 1 << index;
                    }
                }
            }

            let result = rom::RegistrationNumber::from(romcode);
            if result.check_crc() {
                devices[num_found] = Some(result);
                num_found += 1;
            }
        }

        self.init(self.offset.into(), 8);
        Ok(num_found)
    }

    pub async fn reset(&mut self) -> bool {
        self.jump(self.onewire_offset_reset_bus.try_into().unwrap());
        // apply pin mask (see pio program)
        ((self.sm.rx().wait_pull().await & 1) == 0)
    }

    pub fn jump(&mut self, address: u8) {
        unsafe { instr::exec_jmp(&mut self.sm, self.offset + address) }
    }
}

fn config<'d, PIO: pio::Instance>(
    prg: &LoadedProgram<'d, PIO>,
    pin: &'d [&'d embassy_rp::pio::Pin<'d, PIO>],
    bits_per_word: u8,
) -> Config<'d, PIO> {
    let mut cfg = Config::default();

    cfg.use_program(prg, pin);
    cfg.shift_in = ShiftConfig {
        auto_fill: true,
        direction: ShiftDirection::Right,
        threshold: bits_per_word,
    };
    cfg.shift_out = ShiftConfig {
        auto_fill: true,
        direction: ShiftDirection::Right,
        threshold: bits_per_word,
    };
    cfg.set_in_pins(pin);

    // 1us per instruction
    // should get from system clock actually
    // 255_u8.into();
    cfg.clock_divider = 133_u8.into();
    cfg
}
