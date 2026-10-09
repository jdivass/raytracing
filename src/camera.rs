use std::f32::consts::FRAC_PI_2;

use raylib::prelude::*;

pub struct Camera {
    pub position: Vector3,
    yaw: f32,
    pitch: f32,
    move_speed: f32,
    mouse_sensitivity: f32,
}

impl Camera {
    pub fn new(position: Vector3) -> Self {
        Self {
            position,
            yaw: -FRAC_PI_2,
            pitch: 0.0,
            move_speed: 3.0,
            mouse_sensitivity: 0.0025,
        }
    }

    pub fn looking_at(position: Vector3, target: Vector3) -> Self {
        let mut camera = Self::new(position);
        let offset_x = target.x - position.x;
        let offset_y = target.y - position.y;
        let offset_z = target.z - position.z;
        let horizontal_distance = offset_x.hypot(offset_z);

        camera.yaw = offset_z.atan2(offset_x);
        camera.pitch = offset_y.atan2(horizontal_distance);
        camera
    }

    pub fn update(&mut self, window: &RaylibHandle, delta_time: f32) -> bool {
        let mouse_delta = window.get_mouse_delta();
        let mut moved = mouse_delta.x != 0.0 || mouse_delta.y != 0.0;
        self.yaw += mouse_delta.x * self.mouse_sensitivity;
        self.pitch = (self.pitch - mouse_delta.y * self.mouse_sensitivity)
            .clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);

        let distance = self.move_speed * delta_time;
        let forward_x = self.yaw.cos();
        let forward_z = self.yaw.sin();
        let right_x = -forward_z;
        let right_z = forward_x;

        if window.is_key_down(KeyboardKey::KEY_W) {
            moved = true;
            self.position.x += forward_x * distance;
            self.position.z += forward_z * distance;
        }
        if window.is_key_down(KeyboardKey::KEY_S) {
            moved = true;
            self.position.x -= forward_x * distance;
            self.position.z -= forward_z * distance;
        }
        if window.is_key_down(KeyboardKey::KEY_D) {
            moved = true;
            self.position.x += right_x * distance;
            self.position.z += right_z * distance;
        }
        if window.is_key_down(KeyboardKey::KEY_A) {
            moved = true;
            self.position.x -= right_x * distance;
            self.position.z -= right_z * distance;
        }
        if window.is_key_down(KeyboardKey::KEY_E) {
            moved = true;
            self.position.y += distance;
        }
        if window.is_key_down(KeyboardKey::KEY_Q) {
            moved = true;
            self.position.y -= distance;
        }
        moved
    }

    pub fn ray_direction(&self, screen_x: f32, screen_y: f32) -> Vector3 {
        let cos_pitch = self.pitch.cos();
        let forward = Vector3::new(
            self.yaw.cos() * cos_pitch,
            self.pitch.sin(),
            self.yaw.sin() * cos_pitch,
        );
        let right = Vector3::new(-self.yaw.sin(), 0.0, self.yaw.cos());
        let up = Vector3::new(
            -self.yaw.cos() * self.pitch.sin(),
            cos_pitch,
            -self.yaw.sin() * self.pitch.sin(),
        );

        Vector3::new(
            forward.x + right.x * screen_x + up.x * screen_y,
            forward.y + right.y * screen_x + up.y * screen_y,
            forward.z + right.z * screen_x + up.z * screen_y,
        )
        .normalize()
    }
}
