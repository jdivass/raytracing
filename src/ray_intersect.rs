
use raylib::prelude::*;
use crate::material::Material;

pub trait RayIntersect {
    fn ray_intersect(&self, ray_origin: &Vector3, ray_direction: &Vector3) -> Option<(Material, f32)>;
}