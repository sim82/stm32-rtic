#![no_main]
#![no_std]

extern crate panic_halt;

extern crate stm32l4xx_hal as hal;
use rtic_stm32::prelude::*;

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
    rcc::{PllConfig, PllDivider},
    spi::Spi,
    stm32,
    timer::{Event, Timer},
};
use hal::{
    gpio::PC13,
    stm32l4::stm32l4x2::{interrupt, Interrupt, NVIC},
};
use heapless::consts::*;
use rtic::cyccnt::U32Ext;
use smart_leds::SmartLedsWrite;
use ssd1306::{mode::GraphicsMode, prelude::*, Builder, I2CDIBuilder};
const REFRESH_DISPLAY_PERIOD: u32 = 8_000_000 / 4;

#[rtic::app(device = hal::stm32, peripherals = true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
    struct Resources {
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
    }

    #[init(schedule = [refresh_display])]
    fn init(mut cx: init::Context) -> init::LateResources {
        cx.device.RCC.cr.write(|w| w.pllsai1on().clear_bit());
        while cx.device.RCC.cr.read().pllsai1rdy().bit_is_set() {}
        cx.device
            .RCC
            .pllsai1cfgr
            .write(|w| w.pllsai1pen().set_bit());
        cx.device.RCC.cr.write(|w| w.pllsai1on().set_bit());

        cx.device.RCC.apb2enr.write(|w| w.sai1en().set_bit());
        // 2. reset it
        cx.device.RCC.apb2rstr.write(|w| w.sai1rst().set_bit());
        cx.device.RCC.apb2rstr.write(|w| w.sai1rst().clear_bit());

        let mut rcc = cx.device.RCC.constrain();
        let mut flash = cx.device.FLASH.constrain();
        let mut pwr = cx.device.PWR.constrain(&mut rcc.apb1r1);
        let mut cp = cx.core;

        // software tasks won't work without this:
        cp.DCB.enable_trace();
        cp.DWT.enable_cycle_counter();

        let clocks = rcc
            .cfgr
            .sysclk(64.mhz())
            .pclk1(16.mhz())
            .pclk2(64.mhz())
            .freeze(&mut flash.acr, &mut pwr);

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

        disp.write("hello world xxx!", None);
        disp.flush().unwrap();
        cx.schedule
            .refresh_display(cx.start + REFRESH_DISPLAY_PERIOD.cycles())
            .unwrap();

        // ============================================================================
        // I2C
        // let mut gpiob = dp.GPIOB.split(&mut rcc.ahb2);
        // led.set_low();
        let mut lrclk = gpiob
            .pb6
            .into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);
        // .into_open_drain_output(&mut gpiob.moder, &mut gpiob.otyper);
        // lrclk.internal_pull_up(&mut gpiob.pupdr, true);
        let lrclk = lrclk.into_af13(&mut gpiob.moder, &mut gpiob.afrl);

        let mut bclk = gpiob
            .pb3
            .into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);
        // .into_open_drain_output(&mut gpiob.moder, &mut gpiob.otyper);
        // bclk_in.internal_pull_up(&mut gpiob.pupdr, true);
        let bclk_in = bclk.into_af13(&mut gpiob.moder, &mut gpiob.afrl);

        let mut data_in = gpiob
            .pb5
            // .into_push_pull_output(&mut gpioc.moder, &mut gpioc.otyper)
            .into_floating_input(&mut gpiob.moder, &mut gpiob.pupdr);
        // data_out.internal_pull_up(&mut gpiob.pupdr, true);
        let data_in = data_in.into_af13(&mut gpiob.moder, &mut gpiob.afrl);

        // // setup CR2
        // cx.device.SAI1.chb.cr2.write(
        //     |w| w.fth().quarter2(), // threshold half
        // );
        // setup frcr
        cx.device.SAI1.chb.frcr.write(|w| unsafe {
            w
                // .fspol()
                //     .rising_edge() // FS is active high
                .fsdef()
                .set_bit() // FS is start of frame and channel indication
                .fsall()
                .bits(15) // FS high for half frame
                .frl()
                .bits(31) // frame is 32bits
                .fspol()
                .rising_edge()
        });

        // setup slotr
        cx.device.SAI1.chb.slotr.write(|w| unsafe {
            w.sloten()
                .bits(0b11) // enable slots 0, 1
                .nbslot()
                .bits(1) // two slots
                .slotsz()
                .data_size() // 16bit per slot
        });
        // setup CR1

        cx.device.SAI1.chb.cr1.write(|w| {
            w.lsbfirst()
                .msb_first() // big endian
                .ds()
                .bit16() // DS = 16bit
                .ckstr()
                .rising_edge()
                .mode()
                .master_rx() // slave rx
                .prtcfg()
                .free()
                .saien()
                .enabled()
        });
        if !cx.device.SAI1.chb.cr1.read().mode().is_master_rx() {
            panic!("not master rx");
        }
        init::LateResources { disp }
    }

    #[task(schedule=[refresh_display], resources = [disp], priority = 1)]
    fn refresh_display(mut cx: refresh_display::Context) {
        let mut text = heapless::String::<U32>::new();
        write!(&mut text, "{:?}", cx.scheduled).unwrap();
        cx.resources.disp.write(&text, Some(3));
        cx.resources.disp.flush().unwrap();
        cx.schedule
            .refresh_display(cx.scheduled + REFRESH_DISPLAY_PERIOD.cycles())
            .unwrap();
    }

    extern "C" {
        fn COMP();
        fn SDMMC1();
    }
};
