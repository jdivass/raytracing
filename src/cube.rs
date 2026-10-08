use raylib::prelude::*;

use crate::material::Material;
use crate::ray_intersect::RayIntersect;

pub struct Cube {
    pub center: Vector3,
    pub size: f32,
    pub material: Material,
}

impl Cube {
    pub fn new(center: Vector3, size: f32, material: Material) -> Self {
        assert!(size > 0.0, "a cube must have a positive size");

        Self {
            center,
            size,
            material,
        }
    }
}

impl RayIntersect for Cube {
    fn ray_intersect(
        &self,
        ray_origin: &Vector3,
        ray_direction: &Vector3,
    ) -> Option<(Material, f32)> {
        let half_size = self.size * 0.5;
        let min = self.center - Vector3::new(half_size, half_size, half_size);
        let max = self.center + Vector3::new(half_size, half_size, half_size);

        let mut t_near = f32::NEG_INFINITY;
        let mut t_far = f32::INFINITY;

        for (origin, direction, minimum, maximum) in [
            (ray_origin.x, ray_direction.x, min.x, max.x),
            (ray_origin.y, ray_direction.y, min.y, max.y),
            (ray_origin.z, ray_direction.z, min.z, max.z),
        ] {
            if direction.abs() < f32::EPSILON {
                if origin < minimum || origin > maximum {
                    return None;
                }
                continue;
            }

            let mut axis_near = (minimum - origin) / direction;
            let mut axis_far = (maximum - origin) / direction;
            if axis_near > axis_far {
                std::mem::swap(&mut axis_near, &mut axis_far);
            }

            t_near = t_near.max(axis_near);
            t_far = t_far.min(axis_far);
            if t_near > t_far {
                return None;
            }
        }

        const MIN_RAY_DISTANCE: f32 = 0.001;
        let t = if t_near > MIN_RAY_DISTANCE {
            t_near
        } else if t_far > MIN_RAY_DISTANCE {
            t_far
        } else {
            return None;
        };

        Some((self.material, t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cube() -> Cube {
        Cube::new(
            Vector3::new(0.0, 0.0, -5.0),
            2.0,
            Material {
                diffuse: Color::WHITE,
            },
        )
    }

    #[test]
    fn hits_the_nearest_face() {
        let hit = test_cube()
            .ray_intersect(&Vector3::new(0.0, 0.0, 0.0), &Vector3::new(0.0, 0.0, -1.0))
            .expect("ray should hit the cube");

        assert!((hit.1 - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rejects_a_ray_outside_a_parallel_slab() {
        let hit =
            test_cube().ray_intersect(&Vector3::new(2.0, 0.0, 0.0), &Vector3::new(0.0, 0.0, -1.0));

        assert!(hit.is_none());
    }

    #[test]
    fn returns_the_exit_face_when_starting_inside() {
        let hit = test_cube()
            .ray_intersect(&Vector3::new(0.0, 0.0, -5.0), &Vector3::new(1.0, 0.0, 0.0))
            .expect("ray should leave the cube");

        assert!((hit.1 - 1.0).abs() < f32::EPSILON);
    }
}
