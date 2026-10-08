// main.rs
#![allow(unused_imports)]
#![allow(dead_code)]

use raylib::prelude::*;
use std::f32::consts::PI;

mod framebuffer;
mod ray_intersect;
mod sphere;
mod material;

use framebuffer::Framebuffer;
use ray_intersect::RayIntersect;
use sphere::Sphere;
use material::Material;

pub fn cast_ray(
    ray_origin: &Vector3,
    ray_direction: &Vector3,
    objects: &[Sphere],
) -> Color {
    let mut closest_t = f32::INFINITY;
    let mut closest_color = Color::new(4, 12, 36, 255);

    for object in objects {
        if let Some((material, t)) = object.ray_intersect(ray_origin, ray_direction) {
            if t < closest_t {
                closest_t = t;
                closest_color = material.diffuse;
            }
        }
    }

    closest_color
}

pub fn render(framebuffer: &mut Framebuffer, objects: &[Sphere]) {
    let width = framebuffer.width as f32;
    let height = framebuffer.height as f32;
    let fov = PI/3.0;
    let aspect_ratio = width / height;
    let perspective_scale = (fov / 2.0).tan();

    for y in 0..framebuffer.height {
        for x in 0..framebuffer.width {
            let screen_x = (2.0 * x as f32) / width - 1.0;   // 0 .. 1
            let screen_y = (2.0 * y as f32) / height - 1.0;

            let screen_x = screen_x * aspect_ratio * perspective_scale;
            let screen_y = screen_y * perspective_scale;
            
            let ray_direction = Vector3::new(screen_x, screen_y, -1.0).normalize();
            let ray_origin = Vector3::new(0.0, 0.0, 0.0);

            //
            let pixel_color = cast_ray(&ray_origin, &ray_direction, objects);
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
        .title("Raytracer Example")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    let mut framebuffer = Framebuffer::new(window_width as u32, window_height as u32);

    framebuffer.set_background_color(Color::new(80, 80, 200, 255));

    let wood = Material {
        diffuse: Color::new(110, 38, 14, 255),
    };

    let buttons = Material {
        diffuse: Color::new(0,0,0,255),
    };

    let paws = Material {
        diffuse: Color::new(225,193,110,255),
    };

    let nose = Material{
        diffuse: Color::new(128,0,0,255),
    };

    let objects = [
    
    //Cabeza
    Sphere {
        center: Vector3::new(0.0, -1.5, -5.0),
        radius: 0.8,
        material: wood,
    },
    // Cuerpo
    Sphere {
        center: Vector3::new(0.0, 0.0, -5.0),
        radius: 1.0,
        material: wood,
    },

    //Orejas
    //Oreja derecha
    Sphere {
        center: Vector3::new(0.5, -1.8, -4.1),
        radius: 0.35,
        material: wood,
    },

    //Oreza izquierda
    Sphere {
        center: Vector3::new(-0.5, -1.8, -4.1),
        radius: 0.35,
        material: wood,
    },
    //Ojos
    //Ojo derecho
    Sphere {
        center: Vector3::new(0.2, -1.42, -4.1),
        radius: 0.15,
        material: buttons,
    },

    //Ojo izqueirdo
    Sphere {
        center: Vector3::new(-0.2, -1.42, -4.1),
        radius: 0.15,
        material: buttons,
    },

    //Nariz
    Sphere {
        center: Vector3::new(0.0, -1.10, -4.1),
        radius: 0.15,
        material: nose,
    },
    //Manos
    //Mano derecha
    Sphere {
        center: Vector3::new(0.85, -0.4, -4.1),
        radius: 0.35,
        material: wood,
    },

    Sphere {
        center: Vector3::new(0.85, -0.4, -3.9),
        radius: 0.25,
        material: paws,
    },
    //Mano izquierda
    Sphere {
        center: Vector3::new(-0.85, -0.4, -4.1),
        radius: 0.35,
        material: wood,
    },
    Sphere {
        center: Vector3::new(-0.85, -0.4, -3.9),
        radius: 0.25,
        material: paws,
    },
    
    //Pie derecho
    Sphere {
        center: Vector3::new(0.5, 1.0, -4.1),
        radius: 0.35,
        material: wood,
    },

    Sphere {
        center: Vector3::new(0.5, 0.95, -3.9),
        radius: 0.25,
        material: paws,
    },
    //Pie izquierdo
    Sphere {
        center: Vector3::new(-0.5, 1.0, -4.1),
        radius: 0.35,
        material: wood,
    },
    Sphere {
        center: Vector3::new(-0.5, 0.95, -3.9),
        radius: 0.25,
        material: paws,
    },
    //Button arriba
    Sphere {
        center: Vector3::new(0.0, 0.3, -4.2),
        radius: 0.20,
        material: buttons,
    },
    //Button abajo
    Sphere {
        center: Vector3::new(0.0, -0.3, -4.2),
        radius: 0.20,
        material: buttons,
    },
];

    while !window.window_should_close() {
        framebuffer.clear();

        render(&mut framebuffer, &objects);

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}