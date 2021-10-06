use embedded_hal::{blocking::delay::DelayMs, PwmPin};
use stm32f0xx_hal::{
    adc::Adc,
    gpio::{gpioa, Analog},
    pwm::{PwmChannels, C1, C4},
    stm32::{TIM2, TIM3},
};

pub struct BrightnessControl {
    words_pwm: PwmChannels<TIM2, C1>,
    minutes_pwm: PwmChannels<TIM3, C4>,
    adc: Adc,
    adc_pin: gpioa::PA0<Analog>,

    words_current: u16,
    minutes_current: u16,
}

impl BrightnessControl {
    pub fn init(
        mut words_pwm: PwmChannels<TIM2, C1>,
        mut minutes_pwm: PwmChannels<TIM3, C4>,
        adc: Adc,
        adc_pin: gpioa::PA0<Analog>,
    ) -> Self {
        words_pwm.set_duty(0);
        words_pwm.enable();
        minutes_pwm.set_duty(0);
        minutes_pwm.enable();

        Self {
            words_pwm,
            minutes_pwm,
            adc,
            adc_pin,
            words_current: 0,
            minutes_current: 0,
        }
    }

    pub fn update(&mut self) -> () {
        let _brightness = self.read_pd();

        self.words_current = self.words_pwm.get_max_duty() / 5;
        self.words_pwm.set_duty(self.words_current);
        self.minutes_current = self.minutes_pwm.get_max_duty() / 5;
        self.minutes_pwm.set_duty(self.minutes_current);
        // TODO: implement proper brightness control based on ADC measurements
    }

    pub fn dim_down<Delay: DelayMs<u8>>(&mut self, delay: &mut Delay) -> () {
        let steps = 10;
        for i in 1..(steps + 1) {
            self.words_pwm
                .set_duty(self.words_current * (steps - i) / steps);
            self.minutes_pwm
                .set_duty(self.minutes_current * (steps - i) / steps);
            delay.delay_ms(20);
        }
    }

    pub fn dim_up<Delay: DelayMs<u8>>(&mut self, delay: &mut Delay) -> () {
        let steps = 10;
        for i in 1..(steps + 1) {
            self.words_pwm.set_duty(self.words_current * i / steps);
            self.minutes_pwm.set_duty(self.minutes_current * i / steps);
            delay.delay_ms(20);
        }
    }

    fn read_pd(&mut self) -> u16 {
        self.adc.read_abs_mv(&mut self.adc_pin)
    }
}
