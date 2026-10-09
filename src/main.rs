#![allow(unused_imports)]
#![allow(dead_code)]

use raylib::prelude::*;
use std::f32::consts::PI;

mod camera;
mod cube;
mod framebuffer;
mod material;
mod ray_intersect;
mod world;

use camera::Camera;
use framebuffer::Framebuffer;
use ray_intersect::RayIntersect;
use world::VoxelWorld;

pub fn cast_ray(ray_origin: &Vector3, ray_direction: &Vector3, world: &VoxelWorld) -> Color {
    let sky = Color::new(85, 142, 212, 255);
    world
        .ray_intersect(ray_origin, ray_direction)
        .map_or(sky, |hit| {
            let color = hit.material.color_at(hit.position, hit.normal);
            if color.a == 255 {
                return color;
            }

            let alpha = color.a as f32 / 255.0;
            Color::new(
                (color.r as f32 * alpha + sky.r as f32 * (1.0 - alpha)) as u8,
                (color.g as f32 * alpha + sky.g as f32 * (1.0 - alpha)) as u8,
                (color.b as f32 * alpha + sky.b as f32 * (1.0 - alpha)) as u8,
                255,
            )
        })
}

pub fn render(framebuffer: &mut Framebuffer, world: &VoxelWorld, camera: &Camera) {
    let width = framebuffer.width as f32;
    let height = framebuffer.height as f32;
    let fov = PI / 3.0;
    let aspect_ratio = width / height;
    let perspective_scale = (fov / 2.0).tan();

    for y in 0..framebuffer.height {
        for x in 0..framebuffer.width {
            let screen_x = (2.0 * x as f32) / width - 1.0; // 0 .. 1
            // Framebuffer Y grows downward, while the camera's up vector grows upward.
            let screen_y = 1.0 - (2.0 * y as f32) / height;

            let screen_x = screen_x * aspect_ratio * perspective_scale;
            let screen_y = screen_y * perspective_scale;

            let ray_direction = camera.ray_direction(screen_x, screen_y);

            let pixel_color = cast_ray(&camera.position, &ray_direction, world);
            framebuffer.set_current_color(pixel_color);
            framebuffer.set_pixel(x, y)
        }
    }
}

fn main() {
    let window_width = 1000;
    let window_height = 1000;

    let (mut window, raylib_thread) = raylib::init()
        .size(window_width, window_height)
        .fullscreen()
        .title("Raytracer Example")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();
    window.disable_cursor();

    // Fullscreen can be larger than the requested startup size.  Render at
    // the actual back-buffer resolution so the texture covers the whole view.
    let mut framebuffer = Framebuffer::new(
        window.get_render_width() as u32,
        window.get_render_height() as u32,
    );

    framebuffer.set_background_color(Color::new(80, 80, 200, 255));

    let world = VoxelWorld::from_schematic(include_bytes!("../assets/lonlonranchclean.schem"))
        .expect("assets/lonlonranch.schem must be a valid Sponge schematic");
    let mut camera = Camera::looking_at(world.camera_start(), world.camera_target());

    while !window.window_should_close() {
        camera.update(&window, window.get_frame_time());
        framebuffer.clear();

        render(&mut framebuffer, &world, &camera);

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
