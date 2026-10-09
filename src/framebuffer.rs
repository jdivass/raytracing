use raylib::prelude::*;

pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    color_buffer: Image,
    background_color: Color,
    current_color: Color,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let color_buffer = Image::gen_image_color(width as i32, height as i32, Color::BLACK);

        Framebuffer {
            width,
            height,
            color_buffer,
            background_color: Color::BLACK,
            current_color: Color::WHITE,
        }
    }

    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
    }

    pub fn set_current_color(&mut self, color: Color) {
        self.current_color = color;
    }

    pub fn clear(&mut self) {
        self.color_buffer =
            Image::gen_image_color(self.width as i32, self.height as i32, self.background_color);
    }

    pub fn set_pixel(&mut self, x: u32, y: u32) {
        if x < self.width && y < self.height {
            self.color_buffer
                .draw_pixel(x as i32, y as i32, self.current_color);
        }
    }

    pub fn set_pixel_color(&mut self, x: u32, y: u32, color: Color) {
        if x < self.width && y < self.height {
            self.color_buffer.draw_pixel(x as i32, y as i32, color);
        }
    }

    pub fn swap_buffers(&self, window: &mut RaylibHandle, raylib_thread: &RaylibThread) {
        let render_width = window.get_render_width() as f32;
        let render_height = window.get_render_height() as f32;
        if let Ok(texture) = window.load_texture_from_image(raylib_thread, &self.color_buffer) {
            let mut renderer = window.begin_drawing(raylib_thread);
            renderer.clear_background(Color::WHITE);
            renderer.draw_texture_pro(
                &texture,
                Rectangle::new(0.0, 0.0, self.width as f32, self.height as f32),
                Rectangle::new(0.0, 0.0, render_width, render_height),
                Vector2::zero(),
                0.0,
                Color::WHITE,
            );
        }
    }
}
