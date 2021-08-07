use bit_field::BitField;
use chrono::{offset::FixedOffset, DateTime, NaiveDate, NaiveDateTime};
use core::convert::TryInto;
use core::{ops::RangeInclusive, time::Duration};
use embedded_hal::timer::{Cancel, CountDown};
use nb;
use nb::Error::WouldBlock;

#[derive(Copy, Clone)]
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

#[derive(PartialEq)]
enum ParseState {
    ExpectingHigh,
    ExpectingLow,
    ExpectingStart,
    InitialSearch,
    ReceiveErrorSearch,
}

#[derive(Eq, PartialEq)]
enum StateUpdate {
    NewSecond,
    NewMinute,
    Zero,
    One,
}

struct DCF77State {
    next_bits: u64,
    curr_bits: u64,
    current_bit: Option<u32>,
    state: ParseState,
}

impl DCF77State {
    fn now(&self) -> nb::Result<NaiveDateTime, Error> {
        if self.state == ParseState::InitialSearch {
            return Err(WouldBlock);
        }

        self.valid()?;

        fn extract_number(bits: u64, fst: usize, tens: usize) -> u32 {
            (bits.get_bits(fst..(fst + 4)) + bits.get_bits((fst + 5)..(fst + 5 + tens)) * 10)
                .try_into()
                .unwrap()
        }

        let minute = extract_number(self.curr_bits, 21, 3);
        let hour = extract_number(self.curr_bits, 29, 2);
        let day = extract_number(self.curr_bits, 36, 2);
        let month = extract_number(self.curr_bits, 45, 1);
        let year = extract_number(self.curr_bits, 50, 4).try_into().unwrap();
        let second = self.current_bit.ok_or(Error::InvalidTime)?;

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
                .map(|bit| self.next_bits.get_bit(bit))
                .fold(false, |acc, x| acc ^ x);
            // we expect an even parity
            if checksum == true {
                return Err(*error);
            }
        }

        Ok(())
    }

    fn parse_amplitude_change(
        &self,
        new_level: bool,
        time_since_last_update: Duration,
    ) -> StateUpdate {
        if new_level {
            if time_since_last_update > Duration::from_millis(150) {
                StateUpdate::One
            } else {
                StateUpdate::Zero
            }
        } else {
            if time_since_last_update > Duration::from_secs(1) {
                StateUpdate::NewMinute
            } else {
                StateUpdate::NewSecond
            }
        }
    }

    fn update(
        &mut self,
        sender_state: bool,
        time_since_last_update: Duration,
    ) -> Result<(), Error> {
        let state = self.parse_amplitude_change(sender_state, time_since_last_update);

        if state == StateUpdate::NewMinute {
            if self.current_bit.is_some() {
                self.curr_bits = self.next_bits;
            }
            self.next_bits = 0;
            self.state = ParseState::ExpectingHigh;
            self.current_bit = Some(0);
        } else {
            let bit = self.current_bit.ok_or(Error::ProtocolError)?;

            if state == StateUpdate::NewSecond {
                if self.state != ParseState::ExpectingLow {
                    self.current_bit = None;
                    return Err(Error::StateChangeError);
                }

                self.current_bit = Some(bit + 1);
                self.state = ParseState::ExpectingHigh;
            } else {
                if self.state != ParseState::ExpectingHigh {
                    return Err(Error::StateChangeError);
                }

                let value = state == StateUpdate::One;
                self.curr_bits.set_bit((bit - 1) as usize, value);
                if bit == 58 {
                    self.state = ParseState::ExpectingStart;
                } else {
                    self.state = ParseState::ExpectingLow;
                }
            }
        }

        Ok(())
    }
}

pub struct DCF77<Timer>
where
    Timer: CountDown,
{
    timer: Timer,
    state: DCF77State,
}

impl<Timer> DCF77<Timer>
where
    Timer: CountDown,
{
    pub fn init(timer: Timer) -> Self {
        //timer.start(1.s());

        DCF77 {
            timer,
            state: DCF77State {
                next_bits: 0,
                curr_bits: 0,
                current_bit: None,
                state: ParseState::InitialSearch,
            },
        }
    }

    pub fn now(&self) -> nb::Result<DateTime<FixedOffset>, Error> {
        Err(nb::Error::Other(Error::InvalidTime))
    }

    fn update_state(
        &mut self,
        state: bool,
        duration_since_last: Duration,
    ) -> nb::Result<(), Error> {
        /* update the internal state based on a new pin change
         */

        Ok(())
    }
}
