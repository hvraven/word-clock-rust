use embedded_hal::PwmPin;
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
        }
    }

    pub fn update(&mut self) -> () {
        let brightness = self.read_pd();

        self.words_pwm.set_duty(self.words_pwm.get_max_duty() / 20);
        self.minutes_pwm
            .set_duty(self.minutes_pwm.get_max_duty() / 20);
        // TODO: implement proper brightness control based on ADC measurements
    }

    fn read_pd(&mut self) -> u16 {
        self.adc.read_abs_mv(&mut self.adc_pin)
    }
}
