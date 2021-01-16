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

pub mod prelude {
    pub use super::Console;
}
