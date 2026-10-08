use raylib::prelude::*;
use crate::material::Material;
use crate::ray_intersect::RayIntersect;

pub struct Sphere {
    pub center: Vector3,
    pub radius: f32,
    pub material: Material,
}

// sphere.rs
impl RayIntersect for Sphere {
    fn ray_intersect(&self, ray_origin: &Vector3, ray_direction: &Vector3) -> Option<(Material, f32)> {
        let oc = *ray_origin - self.center;
        let a = ray_direction.dot(*ray_direction);
        let b = 2.0 * oc.dot(*ray_direction);
        let c = oc.dot(oc) - self.radius * self.radius;
        let discriminant = b * b - 4.0 * a * c;

        if discriminant < 0.0 {
            return None;
        }

        let sqrt_disc = discriminant.sqrt();
        let t0 = (-b - sqrt_disc) / (2.0 * a);
        let t1 = (-b + sqrt_disc) / (2.0 * a);

        let t = if t0 > 0.001 {
            t0
        } else if t1 > 0.001 {
            t1
        } else {
            return None;
        };

        Some((self.material, t))
    }
}