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
use material::Material;
use ray_intersect::RayIntersect;
use world::VoxelWorld;

struct Skybox {
    width: i32,
    height: i32,
    pixels: Vec<Color>,
}

impl Skybox {
    fn from_jpeg(bytes: &[u8]) -> Self {
        let image = Image::load_image_from_mem(".jpg", bytes)
            .expect("./assets/skybox.jpg must be a readable JPEG");
        let width = image.width();
        let height = image.height();
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            for x in 0..width {
                pixels.push(image.get_color(x, y));
            }
        }
        Self { width, height, pixels }
    }

    fn sample(&self, direction: &Vector3, cycle: &DayCycle) -> Color {
        let yaw = direction.x.atan2(direction.z);
        let pitch = direction.y.clamp(-1.0, 1.0).asin();
        let u = (0.5 + yaw / (2.0 * PI)).rem_euclid(1.0);
        let v = (0.5 - pitch / PI).clamp(0.0, 1.0);
        let x = (u * self.width as f32).floor() as i32 % self.width;
        let y = ((v * self.height as f32).floor() as i32).clamp(0, self.height - 1);
        let mut color = self.pixels[(y * self.width + x) as usize];
        if square_celestial_body(direction, cycle.sun_direction, 0.035) && cycle.daylight > 0.02 {
            color = celestial_color(&cycle.sun_material);
        } else if square_celestial_body(direction, cycle.moon_direction, 0.022) {
            color = celestial_color(&cycle.moon_material);
        }
        let night = 0.10 + cycle.daylight * 0.90;
        Color::new(
            (color.r as f32 * night) as u8,
            (color.g as f32 * night) as u8,
            (color.b as f32 * night) as u8,
            255,
        )
    }
}

fn celestial_color(material: &Material) -> Color {
    let brightness = (0.85 + material.emission * 0.3).min(1.5) * material.albedo;
    Color::new(
        (material.diffuse.r as f32 * brightness).min(255.0) as u8,
        (material.diffuse.g as f32 * brightness).min(255.0) as u8,
        (material.diffuse.b as f32 * brightness).min(255.0) as u8,
        255,
    )
}

fn square_celestial_body(direction: &Vector3, center: Vector3, half_size: f32) -> bool {
    let reference_up = if center.y.abs() > 0.98 {
        Vector3::new(0.0, 0.0, 1.0)
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    };
    let right = reference_up.cross(center).normalize();
    let up = center.cross(right).normalize();
    let forward = direction.dot(center);
    if forward <= 0.0 {
        return false;
    }
    let size = half_size.tan() * forward;
    direction.dot(right).abs() <= size && direction.dot(up).abs() <= size
}

struct DayCycle {
    time: f32,
    sun_direction: Vector3,
    moon_direction: Vector3,
    daylight: f32,
    sun_material: Material,
    moon_material: Material,
}

impl DayCycle {
    fn new(time: f32) -> Self {
        let angle = time.rem_euclid(1.0) * 2.0 * PI - PI / 2.0;
        let sun_direction = Vector3::new(angle.cos(), angle.sin(), 0.25).normalize();
        let moon_direction = sun_direction * -1.0;
        let daylight = sun_direction.y.max(0.0).powf(0.65);
        let sun_material = Material::solid(Color::new(255, 188, 20, 255))
            .with_albedo(1.0)
            .with_specular(1.0)
            .with_transparency(0.0)
            .with_emission(1.4)
            .with_reflection(0.02)
            .with_refraction(0.0, 1.0);
        let moon_material = Material::solid(Color::new(195, 211, 255, 255))
            .with_albedo(0.9)
            .with_specular(0.2)
            .with_transparency(0.0)
            .with_emission(0.08)
            .with_reflection(0.12)
            .with_refraction(0.0, 1.0);
        Self { time: time.rem_euclid(1.0), sun_direction, moon_direction, daylight, sun_material, moon_material }
    }

    fn advance(&mut self, amount: f32) {
        *self = Self::new(self.time + amount);
    }
}

fn cast_ray(ray_origin: &Vector3, ray_direction: &Vector3, world: &VoxelWorld, skybox: &Skybox, cycle: &DayCycle, max_bounces: u8) -> Color {
    cast_ray_depth(ray_origin, ray_direction, world, skybox, cycle, 0, max_bounces)
}

fn cast_ray_depth(ray_origin: &Vector3, ray_direction: &Vector3, world: &VoxelWorld, skybox: &Skybox, cycle: &DayCycle, depth: u8, max_bounces: u8) -> Color {
    let sky = skybox.sample(ray_direction, cycle);
    let mut origin = *ray_origin;
    let mut accumulated = [0.0_f32; 3];
    let mut transmittance = 1.0_f32;

    for _ in 0..32 {
        let Some(hit) = world.ray_intersect(&origin, ray_direction) else {
            break;
        };
        let mut color = hit
            .color
            .unwrap_or_else(|| hit.material.color_at(hit.position, hit.normal));
        let shading_normal = hit.material.normal_at(hit.position, hit.normal);
        color.r = (color.r as f32 * hit.material.albedo).clamp(0.0, 255.0) as u8;
        color.g = (color.g as f32 * hit.material.albedo).clamp(0.0, 255.0) as u8;
        color.b = (color.b as f32 * hit.material.albedo).clamp(0.0, 255.0) as u8;
        let emitted = world.emission_at(hit.position, hit.normal);
        let self_emission = hit.material.emission;
        let sun = shading_normal.dot(cycle.sun_direction).max(0.0) * cycle.daylight * cycle.sun_material.emission;
        let moon = shading_normal.dot(cycle.moon_direction).max(0.0) * cycle.moon_material.emission;
        let daylight_r = 0.10 + sun * cycle.sun_material.diffuse.r as f32 / 255.0 + moon;
        let daylight_g = 0.10 + sun * cycle.sun_material.diffuse.g as f32 / 255.0 + moon;
        let daylight_b = 0.10 + sun * cycle.sun_material.diffuse.b as f32 / 255.0 + moon;
        let view_direction = *ray_direction * -1.0;
        let half_vector = (cycle.sun_direction + view_direction).normalize();
        let specular_light = hit.material.specular
            * cycle.sun_material.specular
            * shading_normal.dot(half_vector).max(0.0).powf(24.0)
            * cycle.daylight * cycle.sun_material.emission;
        color.r = (color.r as f32 * daylight_r + emitted[0] + color.r as f32 * self_emission).clamp(0.0, 255.0) as u8;
        color.g = (color.g as f32 * daylight_g + emitted[1] + color.g as f32 * self_emission).clamp(0.0, 255.0) as u8;
        color.b = (color.b as f32 * daylight_b + emitted[2] + color.b as f32 * self_emission).clamp(0.0, 255.0) as u8;
        color.r = (color.r as f32 + 255.0 * specular_light).clamp(0.0, 255.0) as u8;
        color.g = (color.g as f32 + 218.0 * specular_light).clamp(0.0, 255.0) as u8;
        color.b = (color.b as f32 + 90.0 * specular_light).clamp(0.0, 255.0) as u8;
        if depth < max_bounces && (hit.material.reflection > 0.0 || hit.material.refraction > 0.0) {
            let mut normal = shading_normal;
            let entering = ray_direction.dot(normal) < 0.0;
            if !entering { normal = normal * -1.0; }
            let cosine = (-ray_direction.dot(normal)).clamp(0.0, 1.0);
            let ior = hit.material.refractive_index;
            let eta = if entering { 1.0 / ior } else { ior };
            let f0 = ((ior - 1.0) / (ior + 1.0)).powi(2);
            let fresnel = f0 + (1.0 - f0) * (1.0 - cosine).powi(5);
            let reflected_direction = (*ray_direction - normal * (2.0 * ray_direction.dot(normal))).normalize();
            let reflected = cast_ray_depth(
                &(hit.position + normal * 0.003), &reflected_direction, world, skybox, cycle, depth + 1, max_bounces,
            );
            let mut reflection_weight = hit.material.reflection + hit.material.refraction * fresnel;
            let mut transmission_weight = 0.0;
            let mut transmitted = Color::new(0, 0, 0, 255);
            if hit.material.refraction > 0.0 {
                let discriminant = 1.0 - eta * eta * (1.0 - cosine * cosine);
                if discriminant >= 0.0 {
                    let direction = (*ray_direction * eta + normal * (eta * cosine - discriminant.sqrt())).normalize();
                    transmitted = cast_ray_depth(
                        &(hit.position - normal * 0.003), &direction, world, skybox, cycle, depth + 1, max_bounces,
                    );
                    transmission_weight = hit.material.refraction * (1.0 - fresnel);
                } else {
                    reflection_weight += hit.material.refraction;
                }
            }
            let surface_weight = (1.0 - hit.material.reflection - hit.material.refraction).max(0.0);
            let weight_sum = (surface_weight + reflection_weight + transmission_weight).max(f32::EPSILON);
            color.r = ((color.r as f32 * surface_weight + reflected.r as f32 * reflection_weight + transmitted.r as f32 * transmission_weight) / weight_sum).clamp(0.0, 255.0) as u8;
            color.g = ((color.g as f32 * surface_weight + reflected.g as f32 * reflection_weight + transmitted.g as f32 * transmission_weight) / weight_sum).clamp(0.0, 255.0) as u8;
            color.b = ((color.b as f32 * surface_weight + reflected.b as f32 * reflection_weight + transmitted.b as f32 * transmission_weight) / weight_sum).clamp(0.0, 255.0) as u8;
        }
        let alpha = (color.a as f32 / 255.0) * (1.0 - hit.material.transparency);
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

fn render(framebuffer: &mut Framebuffer, world: &VoxelWorld, camera: &FlyCamera, skybox: &Skybox, cycle: &DayCycle, max_bounces: u8) {
    let width_px = framebuffer.width as usize;
    let height_px = framebuffer.height as usize;
    if width_px == 0 || height_px == 0 {
        return;
    }

    let width = width_px as f32;
    let height = height_px as f32;
    let fov = camera.vertical_fov();
    let aspect_ratio = width / height;
    let perspective_scale = (fov / 2.0).tan();

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
                let color = cast_ray(&basis.origin, &ray_direction, world, skybox, cycle, max_bounces);
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
        .title("Raytracer")
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
    let skybox = Skybox::from_jpeg(include_bytes!("./../assets/skybox.jpg"));
    let mut camera = FlyCamera::looking_at(world.camera_start(), world.camera_target());
    let mut last_camera_motion = Instant::now() - Duration::from_secs(1);

    let mut needs_full_render = true;
    let mut day_cycle = DayCycle::new(0.5);

    while !window.window_should_close() {
        let time_delta = window.get_frame_time() * 0.035;
        let mut time_changed = false;
        if window.is_key_down(KeyboardKey::KEY_Z) {
            day_cycle.advance(-time_delta);
            time_changed = true;
        }
        if window.is_key_down(KeyboardKey::KEY_C) {
            day_cycle.advance(time_delta);
            time_changed = true;
        }
        if time_changed {
            last_camera_motion = Instant::now();
            needs_full_render = true;
        }
        if camera.update(&window, window.get_frame_time()) {
            last_camera_motion = Instant::now();
            needs_full_render = true;
        }

        if last_camera_motion.elapsed() < Duration::from_millis(180) {
            let started = Instant::now();
            render(&mut preview_framebuffer, &world, &camera, &skybox, &day_cycle, 1);
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
                render(&mut framebuffer, &world, &camera, &skybox, &day_cycle, 3);
                needs_full_render = false;
            }
            framebuffer.swap_buffers(&mut window, &raylib_thread);
        }
    }
}
