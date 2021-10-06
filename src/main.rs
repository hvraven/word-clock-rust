#![no_std]
#![no_main]
mod brightness;
mod dcf77;
mod display;

use chrono::NaiveTime;
use cortex_m;
use heapless::{consts::*, spsc::Queue};
use nb::block;
use panic_semihosting as _;
use rtcc::Rtcc;
use rtic::app;
use stm32f0xx_hal::{
    adc::Adc,
    counter::CounterTimer,
    delay::Delay,
    gpio::{
        gpiob::{PB6, PB7},
        Alternate, Output, Pin, PushPull, AF0,
    },
    pac::{EXTI, TIM1},
    prelude::*,
    pwm,
    rcc::HSEBypassMode,
    rtc::{Alarm, Event, Rtc},
    serial::{Event::Rxne, Serial},
    stm32::USART1,
    time::U32Ext,
};

pub struct SerialBuffer {
    queue: Queue<u8, U32>,
}

impl SerialBuffer {
    pub fn new() -> SerialBuffer {
        Self {
            queue: Queue::new(),
        }
    }
}

impl core::fmt::Write for SerialBuffer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();

        for byte in bytes {
            match self.queue.enqueue(*byte) {
                Ok(_) => (),
                Err(_) => return Err(core::fmt::Error),
            }
        }

        Ok(())
    }
}

#[app(device=stm32f0xx_hal::pac, peripherals = true)]
const APP: () = {
    struct Resources {
        words: display::WordDisplay<Pin<Output<PushPull>>>,
        minutes: display::MinuteDisplay<Pin<Output<PushPull>>>,
        brightness: brightness::BrightnessControl,
        dcf77: dcf77::DCF77<CounterTimer<TIM1>>,
        rtc: Rtc,
        delay: Delay,
        serial: Serial<USART1, PB6<Alternate<AF0>>, PB7<Alternate<AF0>>>,
        serial_queue: SerialBuffer,
    }

    #[init()]
    fn init(cx: init::Context) -> init::LateResources {
        cortex_m::interrupt::free(move |cs| {
            let cp: cortex_m::Peripherals = cx.core;
            let dp: stm32f0xx_hal::pac::Peripherals = cx.device;

            let mut flash = dp.FLASH;
            let mut rcc = dp
                .RCC
                .configure()
                .sysclk(8.mhz())
                .hse(8.mhz(), HSEBypassMode::NotBypassed)
                .freeze(&mut flash);
            let gpioa = dp.GPIOA.split(&mut rcc);
            let gpiob = dp.GPIOB.split(&mut rcc);
            let gpioc = dp.GPIOC.split(&mut rcc);
            let gpiof = dp.GPIOF.split(&mut rcc);

            // TODO setup synchronisation to DCF77 pulses
            let mut exti = dp.EXTI;
            let mut pwr = dp.PWR;
            let mut rtc = Rtc::open_or_init(&mut rcc, &mut pwr, dp.RTC, 255, 127, false);
            // setup alarm to trigger interrupt on every full minute
            rtc.listen(&mut exti, Event::AlarmA);
            rtc.set_alarm(Alarm::alarm().subseconds(8, 0)).unwrap();

            //let time = rtc.get_time().unwrap();
            //hprintln!("{}:{}:{}", time.hour(), time.minute(), time.second()).unwrap_or(());

            let dcf77_pin = gpiob.pb3.into_pull_up_input(cs);
            // implement gpio interrupt; enable exti for PB3
            let syscfg = dp.SYSCFG;
            syscfg.exticr1.modify(|_, w| unsafe { w.exti3().bits(1) });
            // Set interrupt request mask for line 3
            exti.imr.modify(|_, w| w.mr3().set_bit());
            // Set interrupt rising and falling trigger for line 3
            exti.rtsr.modify(|_, w| w.tr3().set_bit());
            exti.ftsr.modify(|_, w| w.tr3().set_bit());

            let dcf77 = dcf77::DCF77::init(
                CounterTimer::tim1(dp.TIM1, 1.khz(), &mut rcc),
                dcf77_pin.downgrade(),
                true,
            );

            let words_pwm = pwm::tim2(
                dp.TIM2,
                gpioa.pa5.into_alternate_af2(cs),
                &mut rcc,
                20.khz(),
            );

            let minutes_pwm = pwm::tim3(
                dp.TIM3,
                gpioc.pc9.into_alternate_af0(cs),
                &mut rcc,
                150.khz(),
            );

            let delay = Delay::new(cp.SYST, &rcc);

            let pd1_in = gpioa.pa0.into_analog(cs);
            let _pd2_in = gpioc.pc0.into_analog(cs);

            let adc = Adc::new(dp.ADC, &mut rcc);

            let mut bright_ctl =
                brightness::BrightnessControl::init(words_pwm, minutes_pwm, adc, pd1_in);

            let mut word_display = display::WordDisplay::init(
                gpioa.pa6.into_push_pull_output(cs).downgrade(),
                gpiob.pb8.into_push_pull_output(cs).downgrade(),
                gpioc.pc1.into_push_pull_output(cs).downgrade(),
                gpioc.pc7.into_push_pull_output(cs).downgrade(),
                gpioc.pc10.into_push_pull_output(cs).downgrade(),
                gpioc.pc11.into_push_pull_output(cs).downgrade(),
                gpioc.pc12.into_push_pull_output(cs).downgrade(),
                gpioa.pa15.into_push_pull_output(cs).downgrade(),
                gpiob.pb4.into_push_pull_output(cs).downgrade(),
                gpiob.pb5.into_push_pull_output(cs).downgrade(),
                gpiob.pb9.into_push_pull_output(cs).downgrade(),
                gpiob.pb14.into_push_pull_output(cs).downgrade(),
                gpiob.pb15.into_push_pull_output(cs).downgrade(),
                gpiob.pb12.into_push_pull_output(cs).downgrade(),
                gpiob.pb13.into_push_pull_output(cs).downgrade(),
                gpioc.pc8.into_push_pull_output(cs).downgrade(),
                gpiob.pb11.into_push_pull_output(cs).downgrade(),
                gpioa.pa2.into_push_pull_output(cs).downgrade(),
                gpiob.pb10.into_push_pull_output(cs).downgrade(),
                gpioc.pc3.into_push_pull_output(cs).downgrade(),
                gpioc.pc2.into_push_pull_output(cs).downgrade(),
                gpioc.pc6.into_push_pull_output(cs).downgrade(),
                gpioa.pa1.into_push_pull_output(cs).downgrade(),
                gpioa.pa3.into_push_pull_output(cs).downgrade(),
                gpiob.pb1.into_push_pull_output(cs).downgrade(),
                gpiob.pb0.into_push_pull_output(cs).downgrade(),
                gpiof.pf5.into_push_pull_output(cs).downgrade(),
                gpioa.pa7.into_push_pull_output(cs).downgrade(),
                gpiob.pb2.into_push_pull_output(cs).downgrade(),
                gpioc.pc5.into_push_pull_output(cs).downgrade(),
                gpioa.pa4.into_push_pull_output(cs).downgrade(),
            )
            .unwrap();

            let minute_display = display::MinuteDisplay::init([
                gpioa.pa12.into_push_pull_output(cs).downgrade(),
                gpioa.pa10.into_push_pull_output(cs).downgrade(),
                gpioa.pa11.into_push_pull_output(cs).downgrade(),
                gpioa.pa9.into_push_pull_output(cs).downgrade(),
            ])
            .unwrap();
            // TODO implement serial communication
            // TODO setup stdout / stderr to serial
            let serial_queue = SerialBuffer::new();

            let mut serial = Serial::usart1(
                dp.USART1,
                (
                    gpiob.pb6.into_alternate_af0(cs),
                    gpiob.pb7.into_alternate_af0(cs),
                ),
                115_200.bps(),
                &mut rcc,
            );
            serial.listen(Rxne);
            // TODO implement schwaben schalter
            let _schwaben_schalter = gpiof.pf4.into_floating_input(cs);

            let _time = NaiveTime::from_hms(11, 19, 42);
            let time = rtc.get_time().unwrap();

            word_display.test().unwrap();
            word_display.set_time(time).unwrap();
            // minute_display.set_time(time).unwrap();

            bright_ctl.update();

            init::LateResources {
                words: word_display,
                minutes: minute_display,
                brightness: bright_ctl,
                dcf77,
                rtc,
                delay,
                serial,
                serial_queue,
            }
        })
    }

    #[idle]
    fn idle(_cx: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }

    #[task(resources = [serial, serial_queue])]
    fn process_serial(cx: process_serial::Context) {
        while let Some(b) = cx.resources.serial_queue.queue.dequeue() {
            block!(cx.resources.serial.write(b)).unwrap();
        }
    }

    #[task(binds=RTC, resources = [brightness, rtc, words, minutes, serial, delay])]
    fn rtc(cx: rtc::Context) {
        // RTC interrupt triggered on the start of every minute
        let time = cx.resources.rtc.get_time().unwrap();

        //cx.resources.serial.lock(|&mut s| {
        //    write!(s, "{}:{}:{}\n", time.hour(), time.minute(), time.second()).unwrap();
        //});

        //hprintln!("{}:{}:{}", time.hour(), time.minute(), time.second()).unwrap_or(());

        if cx.resources.words.needs_update(time) {
            cx.resources.brightness.dim_down(cx.resources.delay);
            cx.resources.words.set_time(time).unwrap();
            cx.resources.minutes.set_time(time).unwrap();
            cx.resources.delay.delay_ms(250u16);
            cx.resources.brightness.dim_up(cx.resources.delay);
        } else {
            cx.resources.minutes.set_time(time).unwrap();
        }

        // update brightness based on PD light level
        cx.resources.brightness.update();

        cx.resources.rtc.clear_interrupt(Event::AlarmA)
    }

    #[task(binds=EXTI2_3, resources=[dcf77], priority=2)]
    fn dcf77_pin(cx: dcf77_pin::Context) {
        cx.resources.dcf77.update_state().unwrap();

        // clear exti pending bit
        unsafe { (*EXTI::ptr()).pr.write(|w| w.pr3().set_bit()) }
    }

    extern "C" {
        fn I2C1();
    }
};
