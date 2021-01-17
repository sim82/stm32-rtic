#![no_main]
#![no_std]

extern crate panic_halt;

extern crate stm32l4xx_hal as hal;
use rtic_stm32::prelude::*;
use smart_leds::RGB8;
use ws2812::Ws2812;

use core::fmt::Write;
use embedded_graphics::{fonts, pixelcolor, prelude::*, style};
use hal::{
    device::I2C1,
    gpio::gpioa::PA0,
    gpio::{
        Alternate, Edge, Floating, Input, OpenDrain, Output, PullUp, PushPull, PA1, PA5, PA6, PA7,
        PB8, PB9,
    },
    i2c::I2c,
    prelude::*,
    spi::Spi,
    stm32,
    timer::{Event, Timer},
};
use hal::{
    gpio::PC13,
    stm32l4::stm32l4x6::{interrupt, Interrupt, NVIC},
};
use heapless::consts::*;
use rtic::cyccnt::U32Ext;
use smart_leds::SmartLedsWrite;
use ssd1306::{mode::GraphicsMode, prelude::*, Builder, I2CDIBuilder};
use ws2812_spi as ws2812;

const GAMMA8: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 5, 5, 5,
    5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10, 10, 11, 11, 11, 12, 12, 13, 13, 13, 14,
    14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 24, 24, 25, 25, 26, 27,
    27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 35, 35, 36, 37, 38, 39, 39, 40, 41, 42, 43, 44, 45, 46,
    47, 48, 49, 50, 50, 51, 52, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 66, 67, 68, 69, 70, 72,
    73, 74, 75, 77, 78, 79, 81, 82, 83, 85, 86, 87, 89, 90, 92, 93, 95, 96, 98, 99, 101, 102, 104,
    105, 107, 109, 110, 112, 114, 115, 117, 119, 120, 122, 124, 126, 127, 129, 131, 133, 135, 137,
    138, 140, 142, 144, 146, 148, 150, 152, 154, 156, 158, 160, 162, 164, 167, 169, 171, 173, 175,
    177, 180, 182, 184, 186, 189, 191, 193, 196, 198, 200, 203, 205, 208, 210, 213, 215, 218, 220,
    223, 225, 228, 231, 233, 236, 239, 241, 244, 247, 249, 252, 255,
];

const REFRESH_DISPLAY_PERIOD: u32 = 8_000_000 / 4;
const REFRESH_LED_STRIP_PERIOD: u32 = 8_000_000 / 9;

#[rtic::app(device = hal::stm32, peripherals = true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
    struct Resources {
        led: PA1<Output<PushPull>>,
        pwm: hal::pwm::Pwm<hal::pac::TIM2, hal::pwm::C1>,
        button: PC13<Input<PullUp>>,
        timer: Timer<stm32::TIM7>,
        cur: i32,
        max: i32,
        delta: i32,
        is_on: bool,
        is_on2: bool,
        disp: GraphicsMode<
            I2CInterface<
                I2c<
                    I2C1,
                    (
                        PB8<Alternate<hal::gpio::AF4, Output<OpenDrain>>>,
                        PB9<Alternate<hal::gpio::AF4, Output<OpenDrain>>>,
                    ),
                >,
            >,
            DisplaySize128x64,
        >,
        led_strip_dev: ws2812_spi::Ws2812<
            Spi<
                hal::pac::SPI1,
                (
                    PA5<Alternate<hal::gpio::AF5, Input<Floating>>>,
                    PA6<Alternate<hal::gpio::AF5, Input<Floating>>>,
                    PA7<Alternate<hal::gpio::AF5, Input<Floating>>>,
                ),
            >,
        >,
        rainbow: [Rainbow; 8],
        // led_strip_data: [smart_leds::RGB8; 8],
    }

    #[init(schedule = [refresh_display, refresh_led_strip])]
    fn init(mut cx: init::Context) -> init::LateResources {
        let mut rcc = cx.device.RCC.constrain();
        let mut flash = cx.device.FLASH.constrain();
        let mut pwr = cx.device.PWR.constrain(&mut rcc.apb1r1);
        let clocks = rcc
            .cfgr
            .sysclk(64.mhz())
            .pclk1(16.mhz())
            .pclk2(64.mhz())
            .freeze(&mut flash.acr, &mut pwr);

        // ================================================================================
        // Set up LED1
        let mut gpioa = cx.device.GPIOA.split(&mut rcc.ahb2);
        let led = gpioa
            .pa1
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);

        // ================================================================================
        // Set up LED2 on PA0 with pwm
        let led2 = gpioa
            .pa0
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper)
            .into_af1(&mut gpioa.moder, &mut gpioa.afrl);

        let mut pwm = cx.device.TIM2.pwm(led2, 1.khz(), clocks, &mut rcc.apb1r1);

        // pwm.set_max_duty(255);
        let max = pwm.get_max_duty() as i32;

        pwm.enable();

        // ================================================================================
        // Set up button
        let mut gpioc = cx.device.GPIOC.split(&mut rcc.ahb2);
        let mut button = gpioc
            .pc13
            .into_pull_up_input(&mut gpioc.moder, &mut gpioc.pupdr);
        button.make_interrupt_source(&mut cx.device.SYSCFG, &mut rcc.apb2);
        button.enable_interrupt(&mut cx.device.EXTI);
        button.trigger_on_edge(&mut cx.device.EXTI, Edge::Falling);

        // ================================================================================
        // Set up Timer interrupt
        let mut timer = Timer::tim7(cx.device.TIM7, 4.khz(), clocks, &mut rcc.apb1r1);
        timer.listen(Event::TimeOut);

        // ================================================================================
        // set up OLED i2c
        let mut gpiob = cx.device.GPIOB.split(&mut rcc.ahb2);

        let mut scl = gpiob
            .pb8
            .into_open_drain_output(&mut gpiob.moder, &mut gpiob.otyper);
        scl.internal_pull_up(&mut gpiob.pupdr, true);
        let scl = scl.into_af4(&mut gpiob.moder, &mut gpiob.afrh);
        let mut sda = gpiob
            .pb9
            .into_open_drain_output(&mut gpiob.moder, &mut gpiob.otyper);
        sda.internal_pull_up(&mut gpiob.pupdr, true);
        let sda = sda.into_af4(&mut gpiob.moder, &mut gpiob.afrh);

        let mut i2c = I2c::i2c1(
            cx.device.I2C1,
            (scl, sda),
            800.khz(),
            clocks,
            &mut rcc.apb1r1,
        );

        let interface = I2CDIBuilder::new().with_i2c_addr(0x3d).init(i2c);
        let mut disp: GraphicsMode<_, _> = Builder::new()
            // .with_size(DisplaySize::Display128x64NoOffset)
            .connect(interface)
            .into();
        disp.init().unwrap();
        disp.flush().unwrap();

        disp.write("hello world!", None);
        disp.flush().unwrap();
        cx.schedule
            .refresh_display(cx.start + REFRESH_DISPLAY_PERIOD.cycles())
            .unwrap();

        // ================================================================================
        // setup smart-led strip
        let (sck, miso, mosi) = {
            (
                gpioa.pa5.into_af5(&mut gpioa.moder, &mut gpioa.afrl),
                gpioa.pa6.into_af5(&mut gpioa.moder, &mut gpioa.afrl),
                gpioa.pa7.into_af5(&mut gpioa.moder, &mut gpioa.afrl),
            )
        };

        // Configure SPI with 3Mhz rate
        let spi = Spi::spi1(
            cx.device.SPI1,
            (sck, miso, mosi),
            ws2812::MODE,
            3_000_000.hz(),
            clocks,
            &mut rcc.apb2,
        );
        let led_strip_dev = Ws2812::new(spi);

        cx.schedule
            .refresh_led_strip(cx.start + REFRESH_LED_STRIP_PERIOD.cycles())
            .unwrap();

        // Initialization of late resources
        init::LateResources {
            led,
            pwm,
            button,
            timer,
            cur: 0,
            max,
            is_on: false,
            is_on2: false,
            delta: 1,
            disp,
            led_strip_dev,
            rainbow: [
                Rainbow::step_phase(1, 112),
                Rainbow::step_phase(1, 96),
                Rainbow::step_phase(1, 80),
                Rainbow::step_phase(1, 64),
                Rainbow::step_phase(1, 48),
                Rainbow::step_phase(1, 32),
                Rainbow::step_phase(1, 16),
                Rainbow::step_phase(1, 0),
            ],
            //led_strip_data: [rtic_stm32::color::BLACK; 8],
        }
    }

    #[task(binds = TIM7, resources = [timer, pwm, cur, max, delta, is_on2, led], priority = 3)]
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
        while *cx.resources.cur > *cx.resources.max {
            // *cx.resources.delta = -1;
            *cx.resources.cur -= *cx.resources.max;
        }
        while *cx.resources.cur < 0 {
            // *cx.resources.delta = 1;
            *cx.resources.cur += *cx.resources.max;
        }
        //let duty = GAMMA8[*cx.resources.cur as usize] as i32 * *cx.resources.max / 255;
        let duty = *cx.resources.cur;
        cx.resources.pwm.set_duty(duty as u32);
        // cx.resources.pwm.set_duty(*cx.resources.timer);
        // cx.resources.pwm.set_duty(*cx.resources.max);
        *cx.resources.cur += *cx.resources.delta;

        if *cx.resources.is_on2 {
            cx.resources.led.set_low().unwrap();
            *cx.resources.is_on2 = false;
        } else {
            cx.resources.led.set_high().unwrap();
            *cx.resources.is_on2 = true;
        }
    }
    #[task(binds = EXTI15_10, resources = [is_on, button, delta], priority = 2)]
    fn button(mut cx: button::Context) {
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
        let delta = if !*cx.resources.is_on {
            // cx.resources.led.set_high().unwrap();
            *cx.resources.is_on = true;
            1
        } else {
            // cx.resources.led.set_low().unwrap();
            *cx.resources.is_on = false;
            -1
        };

        cx.resources.delta.lock(|x: &mut i32| *x = delta);
    }

    #[task(schedule=[refresh_display], resources = [disp, cur, delta], priority = 1)]
    fn refresh_display(mut cx: refresh_display::Context) {
        // let mut text = String::<U32>::new();
        // for i in (0..8) {
        //     interface.console().write(&text, Some(i));
        // }

        // write!(&mut text, "num: {}", self.i).unwrap();

        let up = cx.resources.delta.lock(|x: &mut i32| *x > 0);
        let cur = cx.resources.cur.lock(|x: &mut i32| *x);

        if up {
            cx.resources.disp.write("up!", Some(1));
        } else {
            cx.resources.disp.write("down!", Some(1));
        }

        let mut text = heapless::String::<U32>::new();
        // for i in (0..8) {
        //     interface.console().write(&text, Some(i));
        // }

        write!(&mut text, "cur: {}", cur).unwrap();
        cx.resources.disp.write(&text, Some(2));

        text.clear();
        write!(&mut text, "{:?}", cx.scheduled).unwrap();
        cx.resources.disp.write(&text, Some(3));
        cx.resources.disp.flush().unwrap();
        cx.schedule
            .refresh_display(cx.scheduled + REFRESH_DISPLAY_PERIOD.cycles())
            .unwrap();
    }
    #[task(schedule=[refresh_led_strip], resources = [led_strip_dev, rainbow], priority = 3)]
    fn refresh_led_strip(cx: refresh_led_strip::Context) {
        cx.resources
            .led_strip_dev
            .write(smart_leds::brightness(
                cx.resources.rainbow.iter_mut().map(|r| r.next().unwrap()),
                32,
            ))
            .unwrap();

        cx.schedule
            .refresh_led_strip(cx.scheduled + REFRESH_LED_STRIP_PERIOD.cycles())
            .unwrap();
    }

    extern "C" {
        fn COMP();
        fn SDMMC1();
    }
};
