#![no_main]
#![no_std]

extern crate panic_halt;

extern crate stm32l4xx_hal as hal;
use hal::{
    gpio::gpioa::PA5,
    gpio::{Input, Output, PullUp, PushPull},
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
        button: PC13<Input<PullUp>>,
        timer: Timer<stm32::TIM2>,
        is_on: bool,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        let mut rcc = cx.device.RCC.constrain();
        let mut flash = cx.device.FLASH.constrain();
        let mut pwr = cx.device.PWR.constrain(&mut rcc.apb1r1);
        let clocks = rcc.cfgr.freeze(&mut flash.acr, &mut pwr);

        // Set up LED
        let mut gpioa = cx.device.GPIOA.split(&mut rcc.ahb2);
        let led = gpioa
            .pa5
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);

        let mut gpioc = cx.device.GPIOC.split(&mut rcc.ahb2);
        let button = gpioc
            .pc13
            .into_pull_up_input(&mut gpioc.moder, &mut gpioc.pupdr);

        // Set up Timer
        let mut timer = Timer::tim2(cx.device.TIM2, 5.hz(), clocks, &mut rcc.apb1r1);
        timer.listen(Event::TimeOut);

        // Initialization of late resources
        init::LateResources {
            led,
            button,
            timer,
            is_on: false,
        }
    }

    #[task(binds = TIM2, resources = [timer, led, is_on])]
    fn tim2(cx: tim2::Context) {
        cx.resources.timer.clear_interrupt(Event::TimeOut);
        if !*cx.resources.is_on {
            cx.resources.led.set_high().unwrap();
            *cx.resources.is_on = true;
        } else {
            cx.resources.led.set_low().unwrap();
            *cx.resources.is_on = false;
        }
    }
    #[task(binds = GPIOC, resources = [timer, led, is_on])]
    fn button(cx: button::Context) {
        cx.resources.timer.clear_interrupt(Event::TimeOut);
        if !*cx.resources.is_on {
            cx.resources.led.set_high().unwrap();
            *cx.resources.is_on = true;
        } else {
            cx.resources.led.set_low().unwrap();
            *cx.resources.is_on = false;
        }
    }
};
