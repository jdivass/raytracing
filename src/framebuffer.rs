use raylib::prelude::*;

pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    background_color: Color,
    current_color: Color,
    texture: Option<Texture2D>,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let mut framebuffer = Framebuffer {
            width,
            height,
            pixels: vec![0; width as usize * height as usize * 4],
            background_color: Color::BLACK,
            current_color: Color::WHITE,
            texture: None,
        };
        framebuffer.clear();
        framebuffer
    }

    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
    }

    pub fn set_current_color(&mut self, color: Color) {
        self.current_color = color;
    }

    pub fn clear(&mut self) {
        let color = self.background_color;
        for pixel in self.pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[color.r, color.g, color.b, color.a]);
        }
    }

    pub fn set_pixel(&mut self, x: u32, y: u32) {
        self.set_pixel_color(x, y, self.current_color);
    }

    pub fn set_pixel_color(&mut self, x: u32, y: u32, color: Color) {
        if x < self.width && y < self.height {
            let offset = (y as usize * self.width as usize + x as usize) * 4;
            self.pixels[offset..offset + 4].copy_from_slice(&[color.r, color.g, color.b, color.a]);
        }
    }

    pub fn swap_buffers(&mut self, window: &mut RaylibHandle, raylib_thread: &RaylibThread) {
        let render_width = window.get_render_width() as f32;
        let render_height = window.get_render_height() as f32;

        if self.texture.is_none() {
            let image = Image::gen_image_color(self.width as i32, self.height as i32, Color::BLACK);
            self.texture = window.load_texture_from_image(raylib_thread, &image).ok();
        }
        let Some(texture) = self.texture.as_mut() else {
            return;
        };
        let _ = texture.update_texture(&self.pixels);

        let mut renderer = window.begin_drawing(raylib_thread);
        renderer.clear_background(Color::WHITE);
        renderer.draw_texture_pro(
            &*texture,
            Rectangle::new(0.0, 0.0, self.width as f32, self.height as f32),
            Rectangle::new(0.0, 0.0, render_width, render_height),
            Vector2::zero(),
            0.0,
            Color::WHITE,
        );
    }
}
