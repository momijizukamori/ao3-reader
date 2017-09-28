extern crate png;

use std::fs::File;
use png::HasParameters;
use framebuffer::{Framebuffer, UpdateMode};
use color::WHITE;
use geom::{Rectangle, lerp};
use errors::*;

pub struct ImageFramebuffer {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl ImageFramebuffer {
    pub fn new(width: u32, height: u32) -> ImageFramebuffer {
        let len = (width * height) as usize;
        ImageFramebuffer {
            width,
            height,
            data: vec![255; len],
        }
    }

    pub fn clear(&mut self, color: u8) {
        let rect = self.rect();
        self.draw_rectangle(&rect, color);
    }
}

impl Framebuffer for ImageFramebuffer {
    fn set_pixel(&mut self, x: u32, y: u32, color: u8) {
        let addr = (y * self.width + x) as usize;
        self.data[addr] = color;
    }

    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32) {
        if alpha == 1.0 {
            self.set_pixel(x, y, color);
            return;
        }
        let addr = (y * self.width + x) as usize;
        let blended_color = lerp(self.data[addr], color, alpha);
        self.data[addr] = blended_color;
    }

    fn update(&mut self, _: &Rectangle, mode: UpdateMode) -> Result<u32> {
        if mode == UpdateMode::Clear {
            self.clear(WHITE);
        }
        Ok(1)
    }

    fn wait(&self, _: u32) -> Result<i32> {
        Ok(1)
    }

    fn save(&self, path: &str) -> Result<()> {
        let (width, height) = self.dims();
        let file = File::create(path).chain_err(|| "Can't create output file")?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set(png::ColorType::Grayscale).set(png::BitDepth::Eight);
        let mut writer = encoder.write_header().chain_err(|| "Can't write header")?;
        writer.write_image_data(&self.data).chain_err(|| "Can't write data to file")?;
        Ok(())
    }

    fn dims(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}