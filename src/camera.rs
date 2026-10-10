use std::f32::consts::FRAC_PI_2;

use raylib::prelude::*;

pub struct FlyCamera {
    pub position: Vector3,
    yaw: f32,
    pitch: f32,
    move_speed: f32,
    mouse_sensitivity: f32,
}

impl FlyCamera {
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

    pub fn basis(&self) -> CameraBasis {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        CameraBasis {
            origin: self.position,
            forward: Vector3::new(cos_yaw * cos_pitch, sin_pitch, sin_yaw * cos_pitch),
            right: Vector3::new(-sin_yaw, 0.0, cos_yaw),
            up: Vector3::new(-cos_yaw * sin_pitch, cos_pitch, -sin_yaw * sin_pitch),
        }
    }

    pub fn ray_direction(&self, screen_x: f32, screen_y: f32) -> Vector3 {
        self.basis().ray_direction(screen_x, screen_y)
    }
}

#[derive(Clone, Copy)]
pub struct CameraBasis {
    pub origin: Vector3,
    pub forward: Vector3,
    pub right: Vector3,
    pub up: Vector3,
}

impl CameraBasis {
    #[inline]
    pub fn ray_direction(&self, screen_x: f32, screen_y: f32) -> Vector3 {
        Vector3::new(
            self.forward.x + self.right.x * screen_x + self.up.x * screen_y,
            self.forward.y + self.right.y * screen_x + self.up.y * screen_y,
            self.forward.z + self.right.z * screen_x + self.up.z * screen_y,
        )
        .normalize()
    }
}
