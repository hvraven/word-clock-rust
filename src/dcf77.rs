use bit_field::BitField;
use chrono::{NaiveDate, NaiveDateTime};
use core::convert::TryInto;
use core::ops::RangeInclusive;
use cortex_m_semihosting::hprintln;
use embedded_hal::digital::v2::InputPin;
use nb;
use nb::Error::WouldBlock;
use replace_with::replace_with;
use stm32f0xx_hal::{
    counter::Counter,
    gpio::{Input, Pin, PullUp},
};

#[derive(Copy, Clone, Debug)]
pub enum Error {
    StartNotFound,
    StateChangeError,
    ProtocolError,
    InvalidTime,
    InvalidDate,
    ParityErrorMinute,
    ParityErrorHour,
    ParityErrorDate,
}

struct DCF77Parser<Tim: Counter, S> {
    current_bits: u64,
    next_bits: u64,
    timer: Tim,
    state: S,
}

struct Unknown {}
struct AwaitingLow {
    bit: usize,
}
struct AwaitingHigh {
    bit: usize,
}

impl<Tim: Counter, S> DCF77Parser<Tim, S> {
    fn start_minute(self) -> DCF77Parser<Tim, AwaitingHigh> {
        DCF77Parser {
            current_bits: self.next_bits,
            next_bits: 0,
            timer: self.timer,
            state: AwaitingHigh { bit: 0 },
        }
    }
}

impl<Tim: Counter> DCF77Parser<Tim, Unknown> {
    fn new(timer: Tim) -> Self {
        Self {
            current_bits: 0,
            next_bits: 0,
            timer,
            state: Unknown {},
        }
    }
}

impl<Tim: Counter> DCF77Parser<Tim, AwaitingHigh> {
    fn update(mut self, bit: bool) -> DCF77Parser<Tim, AwaitingLow> {
        self.next_bits.set_bit(self.state.bit, bit);
        DCF77Parser {
            current_bits: self.current_bits,
            next_bits: self.next_bits,
            timer: self.timer,
            state: AwaitingLow {
                bit: self.state.bit,
            },
        }
    }
}

impl<Tim: Counter> From<DCF77Parser<Tim, AwaitingHigh>> for DCF77Parser<Tim, Unknown> {
    fn from(old: DCF77Parser<Tim, AwaitingHigh>) -> Self {
        DCF77Parser {
            current_bits: old.current_bits,
            next_bits: 0,
            timer: old.timer,
            state: Unknown {},
        }
    }
}

impl<Tim: Counter> From<DCF77Parser<Tim, AwaitingLow>> for DCF77Parser<Tim, AwaitingHigh> {
    fn from(old: DCF77Parser<Tim, AwaitingLow>) -> Self {
        DCF77Parser {
            current_bits: old.current_bits,
            next_bits: old.next_bits,
            timer: old.timer,
            state: AwaitingHigh {
                bit: old.state.bit + 1,
            },
        }
    }
}

impl<Tim: Counter> From<DCF77Parser<Tim, AwaitingLow>> for DCF77Parser<Tim, Unknown> {
    fn from(old: DCF77Parser<Tim, AwaitingLow>) -> Self {
        DCF77Parser {
            current_bits: old.current_bits,
            next_bits: 0,
            timer: old.timer,
            state: Unknown {},
        }
    }
}

enum DCF77StateWrapper<Tim: Counter> {
    Unknown(DCF77Parser<Tim, Unknown>),
    AwaitingLow(DCF77Parser<Tim, AwaitingLow>),
    AwaitingHigh(DCF77Parser<Tim, AwaitingHigh>),
}

impl<Tim: Counter> DCF77StateWrapper<Tim> {
    pub fn new(timer: Tim) -> Self {
        DCF77StateWrapper::Unknown(DCF77Parser::new(timer))
    }

    pub fn current_bits(&self) -> u64 {
        match self {
            DCF77StateWrapper::Unknown(dcf77) => dcf77.current_bits,
            DCF77StateWrapper::AwaitingHigh(dcf77) => dcf77.current_bits,
            DCF77StateWrapper::AwaitingLow(dcf77) => dcf77.current_bits,
        }
    }

    pub fn update(self, rising_edge: bool) -> Self {
        if rising_edge {
            // going up, end of data.
            match self {
                DCF77StateWrapper::Unknown(mut dcf77) => {
                    //hprintln!("?").unwrap_or(());
                    dcf77.timer.restart();
                    DCF77StateWrapper::Unknown(dcf77)
                }
                DCF77StateWrapper::AwaitingHigh(mut dcf77) => {
                    let time_ms = dcf77.timer.restart();
                    if time_ms < 150 {
                        //hprintln!("0").unwrap_or(());
                        DCF77StateWrapper::AwaitingLow(dcf77.update(false))
                    } else if time_ms < 250 {
                        //hprintln!("1").unwrap_or(());
                        DCF77StateWrapper::AwaitingLow(dcf77.update(true))
                    } else {
                        DCF77StateWrapper::Unknown(dcf77.into())
                    }
                }
                DCF77StateWrapper::AwaitingLow(mut dcf77) => {
                    dcf77.timer.restart();
                    DCF77StateWrapper::Unknown(dcf77.into())
                }
            }
        } else {
            // going down, begin of new second, begin of data
            match self {
                DCF77StateWrapper::Unknown(mut dcf77) => {
                    let time = dcf77.timer.restart();
                    if time > 1800 && time < 2200 {
                        DCF77StateWrapper::AwaitingHigh(dcf77.start_minute())
                    } else {
                        DCF77StateWrapper::Unknown(dcf77)
                    }
                }
                DCF77StateWrapper::AwaitingLow(mut dcf77) => {
                    let time = dcf77.timer.restart();
                    if time > 1800 && time < 2200 {
                        DCF77StateWrapper::AwaitingHigh(dcf77.start_minute())
                    } else {
                        DCF77StateWrapper::AwaitingHigh(dcf77.into())
                    }
                }
                DCF77StateWrapper::AwaitingHigh(mut dcf77) => {
                    dcf77.timer.restart();
                    DCF77StateWrapper::Unknown(dcf77.into())
                }
            }
        }
    }
}

pub struct DCF77<Timer: Counter> {
    state: DCF77StateWrapper<Timer>,
    pin: Pin<Input<PullUp>>,
    inverted: bool,
}

impl<Timer: Counter> DCF77<Timer> {
    pub fn init(timer: Timer, pin: Pin<Input<PullUp>>, inverted: bool) -> Self {
        DCF77 {
            state: DCF77StateWrapper::new(timer),
            pin,
            inverted,
        }
    }

    pub fn update_state(&mut self) -> Result<(), Error> {
        let rising_edge = self.pin.is_high().unwrap() ^ self.inverted;

        replace_with(
            &mut self.state,
            || panic!(""),
            |state| state.update(rising_edge),
        );

        Ok(())
    }

    pub fn now(&self) -> nb::Result<NaiveDateTime, Error> {
        let second = match &self.state {
            DCF77StateWrapper::Unknown(_) => return Err(WouldBlock),
            DCF77StateWrapper::AwaitingHigh(dcf77) => dcf77.state.bit,
            DCF77StateWrapper::AwaitingLow(dcf77) => dcf77.state.bit,
        } as u32;

        self.valid()?;

        fn extract_number(bits: u64, fst: usize, tens: usize) -> u32 {
            (bits.get_bits(fst..(fst + 4)) + bits.get_bits((fst + 5)..(fst + 5 + tens)) * 10)
                .try_into()
                .unwrap()
        }

        let curr_bits = self.state.current_bits();

        let minute = extract_number(curr_bits, 21, 3);
        let hour = extract_number(curr_bits, 29, 2);
        let day = extract_number(curr_bits, 36, 2);
        let month = extract_number(curr_bits, 45, 1);
        let year = extract_number(curr_bits, 50, 4).try_into().unwrap();

        let date = NaiveDate::from_ymd_opt(year, month, day).ok_or(Error::InvalidDate)?;
        date.and_hms_opt(hour, minute, second)
            .ok_or(nb::Error::Other(Error::InvalidTime))
    }

    fn valid(&self) -> Result<(), Error> {
        const PARITY_RANGES: [(RangeInclusive<usize>, Error); 3] = [
            (21..=28, Error::ParityErrorMinute),
            (29..=35, Error::ParityErrorHour),
            (36..=58, Error::ParityErrorDate),
        ];

        for (bit_range, error) in PARITY_RANGES.iter() {
            let checksum = bit_range
                .clone()
                .map(|bit| self.state.current_bits().get_bit(bit))
                .fold(false, |acc, x| acc ^ x);
            // we expect an even parity
            if checksum == true {
                return Err(*error);
            }
        }

        Ok(())
    }
}
