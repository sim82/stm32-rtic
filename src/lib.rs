#![no_std]
#![feature(min_const_generics)]
#![feature(slice_fill)]

use embedded_graphics::{fonts, pixelcolor, prelude::*, primitives, style};
use ssd1306::{displaysize::DisplaySize, mode::GraphicsMode, prelude::WriteOnlyDataCommand};

pub trait Console {
    fn write(&mut self, t: &str, line: Option<i32>);
}

impl core::fmt::Write for &mut dyn Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write(s, None);
        Ok(())
    }
}

impl<DI, DSIZE> Console for GraphicsMode<DI, DSIZE>
where
    DSIZE: DisplaySize,
    DI: WriteOnlyDataCommand,
{
    fn write(&mut self, t: &str, line: Option<i32>) {
        // self.clear();
        let style = style::PrimitiveStyleBuilder::new()
            .stroke_width(1)
            .stroke_color(pixelcolor::BinaryColor::Off)
            .fill_color(pixelcolor::BinaryColor::Off)
            .build();

        let y = match line {
            Some(l) => l * 8,
            None => 0,
        };

        primitives::Rectangle::new(Point::new(0, y), Point::new(127, y + 7))
            .into_styled(style)
            .draw(self)
            .unwrap();
        fonts::Text::new(t, Point::new(0, y))
            .into_styled(style::TextStyle::new(
                fonts::Font6x8,
                pixelcolor::BinaryColor::On,
            ))
            .draw(self)
            .unwrap();
        // self.flush().unwrap();
    }
}

pub mod color {
    use smart_leds::RGB8;

    pub struct Rainbow {
        pos: u8,
        step: u8,
    }

    impl Default for Rainbow {
        fn default() -> Self {
            Rainbow { pos: 0, step: 1 }
        }
    }

    impl Rainbow {
        pub fn step(step: u8) -> Self {
            Rainbow { pos: 0, step }
        }
        pub fn step_phase(step: u8, pos: u8) -> Self {
            Rainbow { pos, step }
        }
    }

    impl Iterator for Rainbow {
        type Item = RGB8;

        fn next(&mut self) -> Option<Self::Item> {
            let c = wheel(self.pos);
            self.pos = self.pos.overflowing_add(self.step).0;
            Some(c)
        }
    }
    /// Input a value 0 to 255 to get a color value
    /// The colours are a transition r - g - b - back to r.
    pub fn wheel(mut wheel_pos: u8) -> RGB8 {
        wheel_pos = 255 - wheel_pos;
        if wheel_pos < 85 {
            return (255 - wheel_pos * 3, 0, wheel_pos * 3).into();
        }
        if wheel_pos < 170 {
            wheel_pos -= 85;
            return (0, wheel_pos * 3, 255 - wheel_pos * 3).into();
        }
        wheel_pos -= 170;
        (wheel_pos * 3, 255 - wheel_pos * 3, 0).into()
    }

    pub const BLACK: RGB8 = RGB8 { r: 0, g: 0, b: 0 };
    pub const RED: RGB8 = RGB8 { r: 255, g: 0, b: 0 };
    pub const GREEN: RGB8 = RGB8 { r: 0, g: 255, b: 0 };
    pub const BLUE: RGB8 = RGB8 { r: 0, g: 0, b: 255 };
    pub const CYAN: RGB8 = RGB8 {
        r: 0,
        g: 255,
        b: 255,
    };
    pub const MAGENTA: RGB8 = RGB8 {
        r: 255,
        g: 0,
        b: 255,
    };
    pub const YELLOW: RGB8 = RGB8 {
        r: 255,
        g: 255,
        b: 0,
    };
}

pub mod prelude {
    pub use super::{
        color::{wheel, Rainbow},
        Console,
    };
}
