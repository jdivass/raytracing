#![allow(unused_imports)]
#![allow(dead_code)]

use raylib::prelude::*;
use rayon::prelude::*;
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
    let mut origin = *ray_origin;
    let mut accumulated = [0.0_f32; 3];
    let mut transmittance = 1.0_f32;

    for _ in 0..32 {
        let Some(hit) = world.ray_intersect(&origin, ray_direction) else {
            break;
        };
        let color = hit
            .color
            .unwrap_or_else(|| hit.material.color_at(hit.position, hit.normal));
        let alpha = color.a as f32 / 255.0;
        accumulated[0] += color.r as f32 * alpha * transmittance;
        accumulated[1] += color.g as f32 * alpha * transmittance;
        accumulated[2] += color.b as f32 * alpha * transmittance;
        transmittance *= 1.0 - alpha;
        if transmittance < 0.01 {
            break;
        }
        origin = hit.position + *ray_direction * 0.002;
    }

    Color::new(
        (accumulated[0] + sky.r as f32 * transmittance) as u8,
        (accumulated[1] + sky.g as f32 * transmittance) as u8,
        (accumulated[2] + sky.b as f32 * transmittance) as u8,
        255,
    )
}

pub fn render(framebuffer: &mut Framebuffer, world: &VoxelWorld, camera: &Camera) {
    let width_px = framebuffer.width as usize;
    let height_px = framebuffer.height as usize;
    if width_px == 0 || height_px == 0 {
        return;
    }

    let width = width_px as f32;
    let height = height_px as f32;
    let fov = PI / 3.0;
    let aspect_ratio = width / height;
    let perspective_scale = (fov / 2.0).tan();

    let mut pixels = vec![Color::BLACK; width_px * height_px];
    pixels
        .par_chunks_mut(width_px)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, pixel) in row.iter_mut().enumerate() {
                let screen_x = (2.0 * x as f32) / width - 1.0;
                let screen_y = 1.0 - (2.0 * y as f32) / height;
                let screen_x = screen_x * aspect_ratio * perspective_scale;
                let screen_y = screen_y * perspective_scale;
                let ray_direction = camera.ray_direction(screen_x, screen_y);
                *pixel = cast_ray(&camera.position, &ray_direction, world);
            }
        });

    for (index, color) in pixels.into_iter().enumerate() {
        framebuffer.set_pixel_color((index % width_px) as u32, (index / width_px) as u32, color);
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
