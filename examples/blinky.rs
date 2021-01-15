#![no_main]
#![no_std]

extern crate panic_halt;

extern crate stm32l4xx_hal as hal;
use hal::{
    gpio::gpioa::PA0,
    gpio::{Alternate, Edge, Input, Output, PullUp, PushPull, PA5},
    prelude::*,
    stm32,
    timer::{Event, Timer},
};
use hal::{
    gpio::PC13,
    stm32l4::stm32l4x6::{interrupt, Interrupt, NVIC},
};

#[rtic::app(device = hal::stm32, peripherals = true)]
const APP: () = {
    struct Resources {
        led: PA5<Output<PushPull>>,
        pwm: hal::pwm::Pwm<hal::pac::TIM2, hal::pwm::C1>,
        button: PC13<Input<PullUp>>,
        timer: Timer<stm32::TIM7>,
        cur: u32,
        max: u32,
        is_on: bool,
    }

    #[init]
    fn init(mut cx: init::Context) -> init::LateResources {
        let mut rcc = cx.device.RCC.constrain();
        let mut flash = cx.device.FLASH.constrain();
        let mut pwr = cx.device.PWR.constrain(&mut rcc.apb1r1);
        let clocks = rcc.cfgr.freeze(&mut flash.acr, &mut pwr);

        // Set up LED
        let mut gpioa = cx.device.GPIOA.split(&mut rcc.ahb2);
        let led = gpioa
            .pa5
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);

        let led2 = gpioa
            .pa0
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper)
            .into_af1(&mut gpioa.moder, &mut gpioa.afrl);

        let mut pwm = cx.device.TIM2.pwm(led2, 1.khz(), clocks, &mut rcc.apb1r1);

        let max = pwm.get_max_duty();

        pwm.enable();

        let mut gpioc = cx.device.GPIOC.split(&mut rcc.ahb2);
        let mut button = gpioc
            .pc13
            .into_pull_up_input(&mut gpioc.moder, &mut gpioc.pupdr);
        button.make_interrupt_source(&mut cx.device.SYSCFG, &mut rcc.apb2);
        button.enable_interrupt(&mut cx.device.EXTI);
        button.trigger_on_edge(&mut cx.device.EXTI, Edge::Falling);
        // Set up Timer
        let mut timer = Timer::tim7(cx.device.TIM7, 160.hz(), clocks, &mut rcc.apb1r1);
        timer.listen(Event::TimeOut);

        // Initialization of late resources
        init::LateResources {
            led,
            pwm,
            button,
            timer,
            cur: 0,
            max,
            is_on: false,
        }
    }

    #[task(binds = TIM7, resources = [timer, pwm, cur, max])]
    fn tim7(cx: tim7::Context) {
        cx.resources.timer.clear_interrupt(Event::TimeOut);
        // if !*cx.resources.is_on {
        //     cx.resources.led.set_high().unwrap();
        //     *cx.resources.is_on = true;
        // } else {
        //     cx.resources.led.set_low().unwrap();
        //     *cx.resources.is_on = false;
        // }
        cx.resources.timer;
        if *cx.resources.cur >= *cx.resources.max {
            *cx.resources.cur = 0;
        }
        cx.resources.pwm.set_duty(*cx.resources.cur);
        // cx.resources.pwm.set_duty(*cx.resources.timer);
        // cx.resources.pwm.set_duty(*cx.resources.max);
        *cx.resources.cur += 100;
    }
    #[task(binds = EXTI15_10, resources = [led, is_on, button])]
    fn button(cx: button::Context) {
        // cx.resources.timer.clear_interrupt(Event::TimeOut);
        //cx.resourcescx.resources.button.is_high()
        // *cx.resources.is_on = !*cx.resources.is_on;

        // if cx.resources.button.is_high().unwrap() {
        //     return;
        // }
        if cx.resources.button.check_interrupt() {
            // if we don't clear this bit, the ISR would trigger indefinitely
            cx.resources.button.clear_interrupt_pending_bit();
        }
        if !*cx.resources.is_on {
            cx.resources.led.set_high().unwrap();
            *cx.resources.is_on = true;
        } else {
            cx.resources.led.set_low().unwrap();
            *cx.resources.is_on = false;
        }
    }
};
