use bitflags::bitflags;
use chrono::{NaiveTime, Timelike};
use cortex_m::asm::delay;
use embedded_hal::digital::v2::OutputPin;

bitflags! {
    struct DriverLine : u8 {
        const LINE1A = 0x01;
        const LINE1B = 0x02;
        const LINE2A = 0x04;
        const LINE2B = 0x08;
        const LINE3 = 0x10;
        const LINE4 = 0x20;
        const LINE5A = 0x40;
        const LINE5B = 0x80;
    }
}

bitflags! {
    struct MainWord : u32 {
        const ES_IST = 0x01;
        const FUENF = 0x02;
        const ZEHN  = 0x04;
        const ZWANZIG = 0x08;
        const DREI = 0x10;
        const VIERTEL = 0x20;
        const VOR = 0x40;
        const NACH = 0x80;
        const HALB = 0x100;
        const UHR = 0x200;
    }
}

struct TimeState {
    main: MainWord,
    next_hour: bool,
}

const OFF_STATE: TimeState = TimeState {
    main: MainWord::empty(),
    next_hour: false,
};

const FIVE_MINUTE_STATE: [TimeState; 12] = [
    TimeState {
        main: MainWord::from_bits_truncate(MainWord::ES_IST.bits() | MainWord::UHR.bits()),
        next_hour: false,
    },
    TimeState {
        main: MainWord::from_bits_truncate(MainWord::FUENF.bits() | MainWord::NACH.bits()),
        next_hour: false,
    },
    TimeState {
        main: MainWord::from_bits_truncate(MainWord::ZEHN.bits() | MainWord::NACH.bits()),
        next_hour: false,
    },
    TimeState {
        main: MainWord::from_bits_truncate(MainWord::VIERTEL.bits() | MainWord::NACH.bits()),
        next_hour: false,
    },
    TimeState {
        main: MainWord::from_bits_truncate(MainWord::ZWANZIG.bits() | MainWord::NACH.bits()),
        next_hour: false,
    },
    TimeState {
        main: MainWord::from_bits_truncate(
            MainWord::FUENF.bits() | MainWord::VOR.bits() | MainWord::HALB.bits(),
        ),
        next_hour: true,
    },
    TimeState {
        main: MainWord::from_bits_truncate(MainWord::ES_IST.bits() | MainWord::HALB.bits()),
        next_hour: true,
    },
    TimeState {
        main: MainWord::from_bits_truncate(
            MainWord::FUENF.bits() | MainWord::NACH.bits() | MainWord::HALB.bits(),
        ),
        next_hour: true,
    },
    TimeState {
        main: MainWord::from_bits_truncate(
            MainWord::ZEHN.bits() | MainWord::NACH.bits() | MainWord::HALB.bits(),
        ),
        next_hour: true,
    },
    TimeState {
        main: MainWord::from_bits_truncate(
            MainWord::ES_IST.bits() | MainWord::DREI.bits() | MainWord::VIERTEL.bits(),
        ),
        next_hour: true,
    },
    TimeState {
        main: MainWord::from_bits_truncate(MainWord::ZEHN.bits() | MainWord::VOR.bits()),
        next_hour: true,
    },
    TimeState {
        main: MainWord::from_bits_truncate(MainWord::FUENF.bits() | MainWord::VOR.bits()),
        next_hour: true,
    },
];

struct Word<Pin: OutputPin> {
    enable: Pin,
    lines: DriverLine,
}

struct Words<Pin: OutputPin> {
    es_ist: Word<Pin>,
    fuenf: Word<Pin>,
    zehn: Word<Pin>,
    zwanzig: Word<Pin>,
    drei: Word<Pin>,
    viertel: Word<Pin>,
    vor: Word<Pin>,
    nach: Word<Pin>,
    halb: Word<Pin>,
    uhr: Word<Pin>,
}

struct Lines<Pin: OutputPin> {
    line1a: Pin,
    line1b: Pin,
    line2a: Pin,
    line2b: Pin,
    line3: Pin,
    line4: Pin,
    line5a: Pin,
    line5b: Pin,
}

pub struct WordDisplay<Pin: OutputPin> {
    enable: Pin,
    words: Words<Pin>,
    hours: [Word<Pin>; 12],
    lines: Lines<Pin>,
    current: NaiveTime,
}

pub struct MinuteDisplay<Pin: OutputPin> {
    minutes: [Pin; 4],
}

macro_rules! set_pin {
    ( $pin:expr, $state:expr ) => {
        if $state {
            $pin.set_high()
        } else {
            $pin.set_low()
        }
    };
}

macro_rules! set_main_word {
    ( $lines:expr, $word:expr, $state:expr ) => {
        if $state {
            $lines |= $word.lines;
            $word.enable.set_low()
        } else {
            $word.enable.set_high()
        }
    };
}

fn update_main_words<Pin: OutputPin>(
    words: &mut Words<Pin>,
    state: &TimeState,
) -> Result<DriverLine, Pin::Error> {
    let mut lines = DriverLine::empty();

    set_main_word!(lines, words.es_ist, state.main.contains(MainWord::ES_IST))?;
    set_main_word!(lines, words.fuenf, state.main.contains(MainWord::FUENF))?;
    set_main_word!(lines, words.zehn, state.main.contains(MainWord::ZEHN))?;
    set_main_word!(lines, words.zwanzig, state.main.contains(MainWord::ZWANZIG))?;
    set_main_word!(lines, words.drei, state.main.contains(MainWord::DREI))?;
    set_main_word!(lines, words.viertel, state.main.contains(MainWord::VIERTEL))?;
    set_main_word!(lines, words.vor, state.main.contains(MainWord::VOR))?;
    set_main_word!(lines, words.nach, state.main.contains(MainWord::NACH))?;
    set_main_word!(lines, words.halb, state.main.contains(MainWord::HALB))?;
    set_main_word!(lines, words.uhr, state.main.contains(MainWord::UHR))?;

    Ok(lines)
}

fn update_hour_words<Pin: OutputPin>(
    hours: &mut [Word<Pin>; 12],
    hour: usize,
) -> Result<(), Pin::Error> {
    for i in 0..12 {
        set_pin!(hours[i].enable, i != hour)?;
    }
    Ok(())
}

fn update_driver_lines<Pin: OutputPin>(
    lines: &mut Lines<Pin>,
    state: DriverLine,
) -> Result<(), Pin::Error> {
    set_pin!(lines.line1a, !state.contains(DriverLine::LINE1A))?;
    set_pin!(lines.line1b, !state.contains(DriverLine::LINE1B))?;
    set_pin!(lines.line2a, !state.contains(DriverLine::LINE2A))?;
    set_pin!(lines.line2b, !state.contains(DriverLine::LINE2B))?;
    set_pin!(lines.line3, !state.contains(DriverLine::LINE3))?;
    set_pin!(lines.line4, !state.contains(DriverLine::LINE4))?;
    set_pin!(lines.line5a, !state.contains(DriverLine::LINE5A))?;
    set_pin!(lines.line5b, !state.contains(DriverLine::LINE5B))?;
    Ok(())
}

impl<Pin: OutputPin> WordDisplay<Pin> {
    pub fn init(
        enable: Pin,
        es_ist: Pin,
        uhr: Pin,
        halb: Pin,
        vor: Pin,
        drei: Pin,
        viertel: Pin,
        nach: Pin,
        zehn: Pin,
        zwanzig: Pin,
        fuenf: Pin,
        h1: Pin,
        h2: Pin,
        h3: Pin,
        h4: Pin,
        h5: Pin,
        h6: Pin,
        h7: Pin,
        h8: Pin,
        h9: Pin,
        h10: Pin,
        h11: Pin,
        h12: Pin,
        line1a: Pin,
        line1b: Pin,
        line2a: Pin,
        line2b: Pin,
        line3: Pin,
        line4: Pin,
        line5a: Pin,
        line5b: Pin,
    ) -> Result<WordDisplay<Pin>, Pin::Error> {
        let mut display = WordDisplay {
            enable,

            words: Words {
                es_ist: Word {
                    enable: es_ist,
                    lines: DriverLine::LINE1A | DriverLine::LINE1B,
                },
                uhr: Word {
                    enable: uhr,
                    lines: DriverLine::LINE4,
                },
                halb: Word {
                    enable: halb,
                    lines: DriverLine::LINE3,
                },
                vor: Word {
                    enable: vor,
                    lines: DriverLine::LINE4,
                },
                drei: Word {
                    enable: drei,
                    lines: DriverLine::LINE3,
                },
                viertel: Word {
                    enable: viertel,
                    lines: DriverLine::LINE2A | DriverLine::LINE2B,
                },
                nach: Word {
                    enable: nach,
                    lines: DriverLine::LINE4,
                },
                zehn: Word {
                    enable: zehn,
                    lines: DriverLine::LINE2A,
                },
                zwanzig: Word {
                    enable: zwanzig,
                    lines: DriverLine::LINE2A | DriverLine::LINE2B,
                },
                fuenf: Word {
                    enable: fuenf,
                    lines: DriverLine::LINE2A,
                },
            },

            hours: [
                Word {
                    enable: h12,
                    lines: DriverLine::LINE5A | DriverLine::LINE5B,
                },
                Word {
                    enable: h1,
                    lines: DriverLine::LINE5A,
                },
                Word {
                    enable: h2,
                    lines: DriverLine::LINE5A,
                },
                Word {
                    enable: h3,
                    lines: DriverLine::LINE5A,
                },
                Word {
                    enable: h4,
                    lines: DriverLine::LINE5A,
                },
                Word {
                    enable: h5,
                    lines: DriverLine::LINE5A,
                },
                Word {
                    enable: h6,
                    lines: DriverLine::LINE5A | DriverLine::LINE5B,
                },
                Word {
                    enable: h7,
                    lines: DriverLine::LINE5A | DriverLine::LINE5B,
                },
                Word {
                    enable: h8,
                    lines: DriverLine::LINE5A,
                },
                Word {
                    enable: h9,
                    lines: DriverLine::LINE5A,
                },
                Word {
                    enable: h10,
                    lines: DriverLine::LINE5A,
                },
                Word {
                    enable: h11,
                    lines: DriverLine::LINE5A,
                },
            ],

            lines: Lines {
                line1a,
                line1b,
                line2a,
                line2b,
                line3,
                line4,
                line5a,
                line5b,
            },

            current: NaiveTime::from_hms(0, 0, 0),
        };

        // set all pins to the off state
        display.enable.set_low()?;
        update_main_words(&mut display.words, &OFF_STATE)?;
        update_hour_words(&mut display.hours, 0)?;
        update_driver_lines(&mut display.lines, DriverLine::all())?;
        display.enable.set_high()?;

        Ok(display)
    }

    pub fn needs_update(&self, time: NaiveTime) -> bool {
        return self.current.hour() != time.hour()
            || self.current.minute() % 5 != time.second() % 5;
    }

    pub fn set_time(&mut self, time: NaiveTime) -> Result<(), Pin::Error> {
        let state = &FIVE_MINUTE_STATE[(time.second() / 5) as usize];
        let hour = ((time.second() + state.next_hour as u32) % 12) as usize;

        let lines = update_main_words(&mut self.words, state)? | self.hours[hour].lines;

        update_hour_words(&mut self.hours, hour)?;
        update_driver_lines(&mut self.lines, lines)?;
        self.enable.set_low()?;
        // assuming 8MHz clock, delay for 2us (must be 2-20us to clear fault)
        delay(200);
        self.enable.set_high()?;

        self.current = time;

        Ok(())
    }

    pub fn test(&mut self) -> Result<(), Pin::Error> {
        update_main_words(&mut self.words, &OFF_STATE)?;
        update_hour_words(&mut self.hours, 13)?;
        set_pin!(self.hours[0].enable, false)?;
        set_pin!(self.hours[2].enable, false)?;
        let lines = self.hours[0].lines | self.hours[2].lines;
        update_driver_lines(&mut self.lines, lines)?;

        self.enable.set_low()?;
        // 2 to 20us delay to reset fault condition (triggered by changing LEDs)
        delay(200);
        self.enable.set_high()?;

        Ok(())
    }
}

impl<Pin: OutputPin> MinuteDisplay<Pin> {
    pub fn init(minutes: [Pin; 4]) -> Result<MinuteDisplay<Pin>, Pin::Error> {
        let mut display = MinuteDisplay { minutes };

        for i in 0..4 {
            display.minutes[i].set_low()?;
        }

        Ok(display)
    }

    pub fn set_time(&mut self, time: NaiveTime) -> Result<(), Pin::Error> {
        for i in 0..4 {
            set_pin!(self.minutes[i], time.minute() % 5 + 1 < i as u32)?;
        }
        Ok(())
    }
}
