#![allow(unused_imports)]
#![allow(dead_code)]

use raylib::prelude::*;
use rayon::prelude::*;
use std::f32::consts::PI;
use std::time::{Duration, Instant};

mod camera;
mod cube;
mod framebuffer;
mod material;
mod ray_intersect;
mod world;

use camera::FlyCamera;
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

pub fn render(framebuffer: &mut Framebuffer, world: &VoxelWorld, camera: &FlyCamera) {
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

    // Todo lo que no depende del píxel se calcula una vez por frame.
    let basis = camera.basis();
    let column_x: Vec<f32> = (0..width_px)
        .map(|x| ((2.0 * x as f32) / width - 1.0) * aspect_ratio * perspective_scale)
        .collect();

    framebuffer
        .pixels
        .par_chunks_mut(width_px * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let screen_y = (1.0 - (2.0 * y as f32) / height) * perspective_scale;
            for (pixel, &screen_x) in row.chunks_exact_mut(4).zip(column_x.iter()) {
                let ray_direction = basis.ray_direction(screen_x, screen_y);
                let color = cast_ray(&basis.origin, &ray_direction, world);
                pixel.copy_from_slice(&[color.r, color.g, color.b, 255]);
            }
        });
}

fn preview_framebuffer_for(window: &RaylibHandle, divisor: u32) -> Framebuffer {
    Framebuffer::new(
        (window.get_render_width() as u32 / divisor).max(1),
        (window.get_render_height() as u32 / divisor).max(1),
    )
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
    let mut preview_divisor: u32 = 3;
    let mut preview_framebuffer = preview_framebuffer_for(&window, preview_divisor);

    framebuffer.set_background_color(Color::new(80, 80, 200, 255));

    let world = VoxelWorld::from_schematic(include_bytes!("../assets/lonlonranchclean.schem"))
        .expect("assets/lonlonranch.schem must be a valid Sponge schematic");
    let mut camera = FlyCamera::looking_at(world.camera_start(), world.camera_target());
    let mut last_camera_motion = Instant::now() - Duration::from_secs(1);

    let mut needs_full_render = true;

    while !window.window_should_close() {
        if camera.update(&window, window.get_frame_time()) {
            last_camera_motion = Instant::now();
            needs_full_render = true;
        }

        if last_camera_motion.elapsed() < Duration::from_millis(180) {
            let started = Instant::now();
            render(&mut preview_framebuffer, &world, &camera);
            let render_ms = started.elapsed().as_secs_f32() * 1000.0;
            preview_framebuffer.swap_buffers(&mut window, &raylib_thread);

            let new_divisor = if render_ms > 33.0 && preview_divisor < 8 {
                preview_divisor + 1
            } else if render_ms < 12.0 && preview_divisor > 2 {
                preview_divisor - 1
            } else {
                preview_divisor
            };
            if new_divisor != preview_divisor {
                preview_divisor = new_divisor;
                preview_framebuffer = preview_framebuffer_for(&window, preview_divisor);
            }
        } else {
            if needs_full_render {
                render(&mut framebuffer, &world, &camera);
                needs_full_render = false;
            }
            framebuffer.swap_buffers(&mut window, &raylib_thread);
        }
    }
}
