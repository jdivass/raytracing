use crate::material::Material;
use raylib::prelude::*;

pub struct RayHit {
    pub material: Material,
    pub distance: f32,
    pub position: Vector3,
    pub normal: Vector3,
}

pub trait RayIntersect {
    fn ray_intersect(&self, ray_origin: &Vector3, ray_direction: &Vector3) -> Option<RayHit>;
}
