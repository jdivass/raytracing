use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

use flate2::read::GzDecoder;
use raylib::prelude::*;
use serde_json::Value;

use crate::material::{Material, Texture, TextureLibrary};
use crate::ray_intersect::{RayHit, RayIntersect};

pub struct VoxelWorld {
    width: usize,
    height: usize,
    length: usize,
    blocks: Vec<Option<usize>>,
    models: Vec<VoxelBlock>,
    min: Vector3,
    max: Vector3,
}

#[derive(Clone)]
struct VoxelBlock {
    material: Material,
    shapes: Vec<BlockShape>,
    faces: Vec<ModelFace>,
    cube_faces: Option<[usize; 6]>,
}

#[derive(Clone)]
struct ModelFace {
    vertices: [Vector3; 4],
    uvs: [Vector2; 4],
    texture: Arc<Texture>,
    normal: Vector3,
    plane_d: f32,
}

#[derive(Clone)]
enum BlockShape {
    Box { min: Vector3, max: Vector3 },
    FlowerCross,
}

impl VoxelWorld {
    pub fn from_schematic(compressed_schematic: &[u8]) -> Result<Self, String> {
        let mut nbt_data = Vec::new();
        GzDecoder::new(compressed_schematic)
            .read_to_end(&mut nbt_data)
            .map_err(|error| format!("could not decompress schematic: {error}"))?;

        let schematic = SchematicNbt::parse(&nbt_data)?;
        let block_count = schematic
            .width
            .checked_mul(schematic.height)
            .and_then(|count| count.checked_mul(schematic.length))
            .ok_or("schematic dimensions are too large")?;

        let mut blocks = vec![None; block_count];
        let assets_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("textures/assets/minecraft");
        let texture_directory = assets_root.join("textures");
        let mut textures = TextureLibrary::new(texture_directory);
        let model_library = ModelLibrary::new(assets_root);
        let mut models = Vec::new();
        let mut palette = HashMap::new();
        for (palette_index, block_state) in schematic.palette {
            let Some(material) = material_for_block(&block_state, &mut textures) else {
                palette.insert(palette_index, None);
                continue;
            };
            let faces = model_library.faces_for_block_state(&block_state, &mut textures);
            let shapes = if faces.is_empty() {
                shapes_for_block(&block_state)
            } else {
                Vec::new()
            };
            let cube_faces = cube_face_slots(&faces);
            let model_index = models.len();
            models.push(VoxelBlock {
                material,
                shapes,
                faces,
                cube_faces,
            });
            palette.insert(palette_index, Some(model_index));
        }

        let palette_indices = decode_varints(&schematic.block_data)?;
        if palette_indices.len() < block_count {
            return Err(format!(
                "schematic contains {} blocks, expected {block_count}",
                palette_indices.len()
            ));
        }
        for y in 0..schematic.height {
            for z in 0..schematic.length {
                for x in 0..schematic.width {
                    let schematic_index =
                        x + z * schematic.width + y * schematic.width * schematic.length;
                    let world_index = schematic_index;
                    blocks[world_index] = palette
                        .get(&palette_indices[schematic_index])
                        .copied()
                        .flatten();
                }
            }
        }

        let min = Vector3::new(
            -(schematic.width as f32) * 0.5,
            0.0,
            -(schematic.length as f32) * 0.5,
        );
        let max = Vector3::new(
            min.x + schematic.width as f32,
            schematic.height as f32,
            min.z + schematic.length as f32,
        );

        Ok(Self {
            width: schematic.width,
            height: schematic.height,
            length: schematic.length,
            blocks,
            models,
            min,
            max,
        })
    }

    pub fn camera_start(&self) -> Vector3 {
        let largest_dimension = self.width.max(self.length) as f32;
        Vector3::new(
            0.0,
            self.height as f32 + largest_dimension * 0.4,
            self.max.z + largest_dimension * 0.45,
        )
    }

    pub fn camera_target(&self) -> Vector3 {
        Vector3::new(0.0, self.height as f32 * 0.3, 0.0)
    }

    fn index(&self, x: isize, y: isize, z: isize) -> Option<usize> {
        if x < 0
            || y < 0
            || z < 0
            || x >= self.width as isize
            || y >= self.height as isize
            || z >= self.length as isize
        {
            return None;
        }

        Some(x as usize + z as usize * self.width + y as usize * self.width * self.length)
    }
}

impl ModelFace {
    fn finalize(&mut self) -> bool {
        let normal = (self.vertices[1] - self.vertices[0]).cross(self.vertices[2] - self.vertices[0]);
        let length = normal.length();
        if !(length > 1e-8) {
            return false;
        }
        self.normal = normal * (1.0 / length);
        self.plane_d = self.normal.dot(self.vertices[0]);
        true
    }

    #[inline]
    fn ray_intersect(
        &self,
        local_origin: Vector3,
        direction: Vector3,
    ) -> Option<(f32, Vector3, Color)> {
        let denominator = self.normal.dot(direction);
        if denominator.abs() < f32::EPSILON {
            return None;
        }
        let distance = (self.plane_d - self.normal.dot(local_origin)) / denominator;
        if distance <= 0.001 {
            return None;
        }
        let normal = if denominator > 0.0 {
            self.normal * -1.0
        } else {
            self.normal
        };
        let point = local_origin + direction * distance;
        let (weights, indices) = barycentric_quad(point, self.vertices)?;
        let (a, b, c) = indices;
        let uv = Vector2::new(
            self.uvs[a].x * weights[0] + self.uvs[b].x * weights[1] + self.uvs[c].x * weights[2],
            self.uvs[a].y * weights[0] + self.uvs[b].y * weights[1] + self.uvs[c].y * weights[2],
        );
        Some((distance, normal, self.texture.sample_uv(uv.x, uv.y)))
    }

    #[inline]
    fn color_at_local(&self, point: Vector3) -> Color {
        let origin = self.vertices[0];
        let edge_u = self.vertices[1] - origin;
        let edge_v = self.vertices[3] - origin;
        let offset = point - origin;
        let s = offset.dot(edge_u) / edge_u.dot(edge_u);
        let t = offset.dot(edge_v) / edge_v.dot(edge_v);
        let u = self.uvs[0].x + (self.uvs[1].x - self.uvs[0].x) * s + (self.uvs[3].x - self.uvs[0].x) * t;
        let v = self.uvs[0].y + (self.uvs[1].y - self.uvs[0].y) * s + (self.uvs[3].y - self.uvs[0].y) * t;
        self.texture.sample_uv(u, v)
    }
}

#[inline]
fn axis_value(vector: Vector3, axis: usize) -> f32 {
    match axis {
        0 => vector.x,
        1 => vector.y,
        _ => vector.z,
    }
}

#[inline]
fn set_axis_value(vector: &mut Vector3, axis: usize, value: f32) {
    match axis {
        0 => vector.x = value,
        1 => vector.y = value,
        _ => vector.z = value,
    }
}

fn full_face_slot(face: &ModelFace) -> Option<usize> {
    const EPSILON: f32 = 1e-4;
    for axis in 0..3 {
        let plane = axis_value(face.vertices[0], axis);
        if !face
            .vertices
            .iter()
            .all(|vertex| (axis_value(*vertex, axis) - plane).abs() < EPSILON)
        {
            continue;
        }
        let side = if plane.abs() < EPSILON {
            0
        } else if (plane - 1.0).abs() < EPSILON {
            1
        } else {
            return None;
        };
        for other in (0..3).filter(|other| *other != axis) {
            let low = face
                .vertices
                .iter()
                .map(|vertex| axis_value(*vertex, other))
                .fold(f32::INFINITY, f32::min);
            let high = face
                .vertices
                .iter()
                .map(|vertex| axis_value(*vertex, other))
                .fold(f32::NEG_INFINITY, f32::max);
            if low.abs() > EPSILON || (high - 1.0).abs() > EPSILON {
                return None;
            }
        }
        return Some(axis * 2 + side);
    }
    None
}

fn cube_face_slots(faces: &[ModelFace]) -> Option<[usize; 6]> {
    let mut slots: [Option<usize>; 6] = [None; 6];
    for (index, face) in faces.iter().enumerate() {
        let slot = full_face_slot(face)?;
        slots[slot].get_or_insert(index);
    }
    Some([
        slots[0]?, slots[1]?, slots[2]?, slots[3]?, slots[4]?, slots[5]?,
    ])
}

struct ModelLibrary {
    assets_root: PathBuf,
}

#[derive(Default)]
struct ResolvedModel {
    textures: HashMap<String, String>,
    elements: Vec<Value>,
}

#[derive(Clone)]
struct ModelReference {
    model: String,
    x_rotation: f32,
    y_rotation: f32,
}

impl ModelLibrary {
    fn new(assets_root: PathBuf) -> Self {
        Self { assets_root }
    }

    fn faces_for_block_state(
        &self,
        block_state: &str,
        textures: &mut TextureLibrary,
    ) -> Vec<ModelFace> {
        let block = block_name(block_state);
        let properties = state_properties(block_state);
        let state_path = self
            .assets_root
            .join("blockstates")
            .join(format!("{block}.json"));
        let Ok(json_text) = std::fs::read_to_string(state_path) else {
            return Vec::new();
        };
        let Ok(blockstate_json) = serde_json::from_str::<Value>(&json_text) else {
            return Vec::new();
        };

        let references = model_references(&blockstate_json, &properties);
        let mut faces = Vec::new();
        for reference in references {
            let Some(model) = self.resolve_model(&reference.model, 0) else {
                continue;
            };
            for element in &model.elements {
                faces.extend(self.element_faces(element, &model.textures, &reference, textures));
            }
        }
        faces
    }

    fn resolve_model(&self, name: &str, depth: usize) -> Option<ResolvedModel> {
        if depth > 32 {
            return None;
        }
        let model_path = name.strip_prefix("minecraft:").unwrap_or(name);
        let path = self
            .assets_root
            .join("models")
            .join(format!("{model_path}.json"));
        let json_text = std::fs::read_to_string(path).ok()?;
        let json = serde_json::from_str::<Value>(&json_text).ok()?;

        let mut resolved = json
            .get("parent")
            .and_then(Value::as_str)
            .and_then(|parent| self.resolve_model(parent, depth + 1))
            .unwrap_or_default();

        if let Some(textures) = json.get("textures").and_then(Value::as_object) {
            for (key, value) in textures {
                if let Some(value) = value.as_str() {
                    resolved.textures.insert(key.clone(), value.to_owned());
                }
            }
        }
        if let Some(elements) = json.get("elements").and_then(Value::as_array) {
            resolved.elements = elements.clone();
        }
        Some(resolved)
    }

    fn element_faces(
        &self,
        element: &Value,
        model_textures: &HashMap<String, String>,
        reference: &ModelReference,
        textures: &mut TextureLibrary,
    ) -> Vec<ModelFace> {
        let Some(from) = json_vec3(element.get("from")) else {
            return Vec::new();
        };
        let Some(to) = json_vec3(element.get("to")) else {
            return Vec::new();
        };
        let element_rotation = element.get("rotation");
        let rotation_axis = element_rotation
            .and_then(|value| value.get("axis"))
            .and_then(Value::as_str)
            .unwrap_or("y");
        let rotation_angle = element_rotation
            .and_then(|value| value.get("angle"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32;
        let rotation_origin = element_rotation
            .and_then(|value| json_vec3(value.get("origin")))
            .unwrap_or(Vector3::new(0.5, 0.5, 0.5));
        let rescale = element_rotation
            .and_then(|value| value.get("rescale"))
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let Some(face_values) = element.get("faces").and_then(Value::as_object) else {
            return Vec::new();
        };
        let mut faces = Vec::new();
        for (direction, face_value) in face_values {
            let Some(texture_ref) = face_value.get("texture").and_then(Value::as_str) else {
                continue;
            };
            let Some(texture_name) = resolve_texture_ref(texture_ref, model_textures) else {
                continue;
            };
            let Some(texture) = textures.load_model_texture(&texture_name) else {
                continue;
            };
            let Some(mut face) = make_model_face(direction, from, to, face_value, texture) else {
                continue;
            };

            for vertex in &mut face.vertices {
                *vertex = rotate_point(
                    *vertex,
                    rotation_origin,
                    rotation_axis,
                    rotation_angle,
                    rescale,
                );
                *vertex = rotate_point(
                    *vertex,
                    Vector3::new(0.5, 0.5, 0.5),
                    "x",
                    reference.x_rotation,
                    false,
                );
                *vertex = rotate_point(
                    *vertex,
                    Vector3::new(0.5, 0.5, 0.5),
                    "y",
                    reference.y_rotation,
                    false,
                );
            }
            if face.finalize() {
                faces.push(face);
            }
        }
        faces
    }
}

fn block_name(block_state: &str) -> &str {
    block_state
        .strip_prefix("minecraft:")
        .unwrap_or(block_state)
        .split('[')
        .next()
        .unwrap_or(block_state)
}

fn state_properties(block_state: &str) -> HashMap<String, String> {
    let Some((_, states)) = block_state.split_once('[') else {
        return HashMap::new();
    };
    states
        .trim_end_matches(']')
        .split(',')
        .filter_map(|state| {
            let (key, value) = state.split_once('=')?;
            Some((key.to_owned(), value.to_owned()))
        })
        .collect()
}

fn model_references(
    blockstate: &Value,
    properties: &HashMap<String, String>,
) -> Vec<ModelReference> {
    let mut references = Vec::new();
    if let Some(variants) = blockstate.get("variants").and_then(Value::as_object) {
        let mut matching = variants
            .iter()
            .filter(|(variant, _)| variant_matches(variant, properties))
            .collect::<Vec<_>>();
        matching.sort_by_key(|(variant, _)| std::cmp::Reverse(variant.split(',').count()));
        if let Some((_, choice)) = matching.first() {
            append_model_references(choice, &mut references);
        }
    }

    if let Some(parts) = blockstate.get("multipart").and_then(Value::as_array) {
        for part in parts {
            let matches = part
                .get("when")
                .is_none_or(|when| when_matches(when, properties));
            if matches {
                if let Some(apply) = part.get("apply") {
                    append_model_references(apply, &mut references);
                }
            }
        }
    }
    references
}

fn append_model_references(value: &Value, references: &mut Vec<ModelReference>) {
    let values = value
        .as_array()
        .map_or_else(|| vec![value], |items| items.iter().collect());
    for value in values {
        if let Some(model) = value.get("model").and_then(Value::as_str) {
            references.push(ModelReference {
                model: model.to_owned(),
                x_rotation: value.get("x").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                y_rotation: value.get("y").and_then(Value::as_f64).unwrap_or(0.0) as f32,
            });
        }
    }
}

fn variant_matches(variant: &str, properties: &HashMap<String, String>) -> bool {
    if variant.is_empty() {
        return true;
    }
    variant.split(',').all(|condition| {
        let Some((key, expected)) = condition.split_once('=') else {
            return false;
        };
        properties
            .get(key)
            .is_some_and(|actual| expected.split('|').any(|choice| choice == actual))
    })
}

fn when_matches(when: &Value, properties: &HashMap<String, String>) -> bool {
    if let Some(options) = when.get("OR").and_then(Value::as_array) {
        return options
            .iter()
            .any(|option| when_matches(option, properties));
    }
    when.as_object().is_some_and(|conditions| {
        conditions.iter().all(|(key, value)| {
            let Some(expected) = value.as_str() else {
                return false;
            };
            properties
                .get(key)
                .is_some_and(|actual| expected.split('|').any(|choice| choice == actual))
        })
    })
}

fn resolve_texture_ref(texture_ref: &str, textures: &HashMap<String, String>) -> Option<String> {
    let mut current = texture_ref;
    for _ in 0..32 {
        if let Some(name) = current.strip_prefix('#') {
            current = textures.get(name)?;
        } else {
            return Some(current.to_owned());
        }
    }
    None
}

fn json_vec3(value: Option<&Value>) -> Option<Vector3> {
    let values = value?.as_array()?;
    Some(Vector3::new(
        values.first()?.as_f64()? as f32 / 16.0,
        values.get(1)?.as_f64()? as f32 / 16.0,
        values.get(2)?.as_f64()? as f32 / 16.0,
    ))
}

fn make_model_face(
    direction: &str,
    low: Vector3,
    high: Vector3,
    face_json: &Value,
    texture: Arc<Texture>,
) -> Option<ModelFace> {
    let (vertices, uv_default) = match direction {
        "down" => (
            [
                Vector3::new(low.x, low.y, low.z),
                Vector3::new(high.x, low.y, low.z),
                Vector3::new(high.x, low.y, high.z),
                Vector3::new(low.x, low.y, high.z),
            ],
            [
                low.x * 16.0,
                (1.0 - high.z) * 16.0,
                high.x * 16.0,
                (1.0 - low.z) * 16.0,
            ],
        ),
        "up" => (
            [
                Vector3::new(low.x, high.y, high.z),
                Vector3::new(high.x, high.y, high.z),
                Vector3::new(high.x, high.y, low.z),
                Vector3::new(low.x, high.y, low.z),
            ],
            [low.x * 16.0, low.z * 16.0, high.x * 16.0, high.z * 16.0],
        ),
        "north" => (
            [
                Vector3::new(low.x, low.y, low.z),
                Vector3::new(low.x, high.y, low.z),
                Vector3::new(high.x, high.y, low.z),
                Vector3::new(high.x, low.y, low.z),
            ],
            [
                (1.0 - high.x) * 16.0,
                (1.0 - high.y) * 16.0,
                (1.0 - low.x) * 16.0,
                (1.0 - low.y) * 16.0,
            ],
        ),
        "south" => (
            [
                Vector3::new(low.x, low.y, high.z),
                Vector3::new(high.x, low.y, high.z),
                Vector3::new(high.x, high.y, high.z),
                Vector3::new(low.x, high.y, high.z),
            ],
            [
                low.x * 16.0,
                (1.0 - high.y) * 16.0,
                high.x * 16.0,
                (1.0 - low.y) * 16.0,
            ],
        ),
        "west" => (
            [
                Vector3::new(low.x, low.y, high.z),
                Vector3::new(low.x, high.y, high.z),
                Vector3::new(low.x, high.y, low.z),
                Vector3::new(low.x, low.y, low.z),
            ],
            [
                low.z * 16.0,
                (1.0 - high.y) * 16.0,
                high.z * 16.0,
                (1.0 - low.y) * 16.0,
            ],
        ),
        "east" => (
            [
                Vector3::new(high.x, low.y, low.z),
                Vector3::new(high.x, high.y, low.z),
                Vector3::new(high.x, high.y, high.z),
                Vector3::new(high.x, low.y, high.z),
            ],
            [
                (1.0 - high.z) * 16.0,
                (1.0 - high.y) * 16.0,
                (1.0 - low.z) * 16.0,
                (1.0 - low.y) * 16.0,
            ],
        ),
        _ => return None,
    };

    let uv = face_json
        .get("uv")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 4)
        .and_then(|values| {
            Some([
                values[0].as_f64()? as f32,
                values[1].as_f64()? as f32,
                values[2].as_f64()? as f32,
                values[3].as_f64()? as f32,
            ])
        })
        .unwrap_or(uv_default);
    let (u1, v1, u2, v2) = (uv[0], uv[1], uv[2], uv[3]);
    let mut uvs = match direction {
        "down" => [
            Vector2::new(u1, v1),
            Vector2::new(u2, v1),
            Vector2::new(u2, v2),
            Vector2::new(u1, v2),
        ],
        "up" => [
            Vector2::new(u1, v2),
            Vector2::new(u2, v2),
            Vector2::new(u2, v1),
            Vector2::new(u1, v1),
        ],
        "north" => [
            Vector2::new(u1, v2),
            Vector2::new(u1, v1),
            Vector2::new(u2, v1),
            Vector2::new(u2, v2),
        ],
        "south" => [
            Vector2::new(u2, v2),
            Vector2::new(u1, v2),
            Vector2::new(u1, v1),
            Vector2::new(u2, v1),
        ],
        "west" => [
            Vector2::new(u2, v2),
            Vector2::new(u2, v1),
            Vector2::new(u1, v1),
            Vector2::new(u1, v2),
        ],
        "east" => [
            Vector2::new(u1, v2),
            Vector2::new(u1, v1),
            Vector2::new(u2, v1),
            Vector2::new(u2, v2),
        ],
        _ => unreachable!(),
    };
    let turns = face_json
        .get("rotation")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_div(90) as usize
        % 4;
    uvs.rotate_left(turns);

    Some(ModelFace {
        vertices,
        uvs,
        texture,
        normal: Vector3::zero(),
        plane_d: 0.0,
    })
}

fn rotate_point(
    point: Vector3,
    origin: Vector3,
    axis: &str,
    angle_degrees: f32,
    rescale: bool,
) -> Vector3 {
    if angle_degrees == 0.0 {
        return point;
    }
    let radians = angle_degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    let mut p = point - origin;
    if rescale {
        let factor = 1.0 / cos.abs().max(0.0001);
        match axis {
            "x" => {
                p.y *= factor;
                p.z *= factor;
            }
            "y" => {
                p.x *= factor;
                p.z *= factor;
            }
            "z" => {
                p.x *= factor;
                p.y *= factor;
            }
            _ => {}
        }
    }
    let rotated = match axis {
        "x" => Vector3::new(p.x, cos * p.y - sin * p.z, sin * p.y + cos * p.z),
        "y" => Vector3::new(cos * p.x - sin * p.z, p.y, sin * p.x + cos * p.z),
        "z" => Vector3::new(cos * p.x - sin * p.y, sin * p.x + cos * p.y, p.z),
        _ => p,
    };
    rotated + origin
}

fn barycentric_quad(
    point: Vector3,
    vertices: [Vector3; 4],
) -> Option<([f32; 3], (usize, usize, usize))> {
    triangle_barycentric(point, vertices[0], vertices[1], vertices[2])
        .map(|weights| (weights, (0, 1, 2)))
        .or_else(|| {
            triangle_barycentric(point, vertices[0], vertices[2], vertices[3])
                .map(|weights| (weights, (0, 2, 3)))
        })
}

fn triangle_barycentric(point: Vector3, a: Vector3, b: Vector3, c: Vector3) -> Option<[f32; 3]> {
    let v0 = b - a;
    let v1 = c - a;
    let v2 = point - a;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let denominator = d00 * d11 - d01 * d01;
    if denominator.abs() < f32::EPSILON {
        return None;
    }
    let v = (d11 * d20 - d01 * d21) / denominator;
    let w = (d00 * d21 - d01 * d20) / denominator;
    let u = 1.0 - v - w;
    (u >= -0.0001 && v >= -0.0001 && w >= -0.0001).then_some([u, v, w])
}

impl RayIntersect for VoxelWorld {
    fn ray_intersect<'a>(
        &'a self,
        ray_origin: &Vector3,
        ray_direction: &Vector3,
    ) -> Option<RayHit<'a>> {
        let (entry, exit, entry_axis) =
            ray_box_interval(*ray_origin, *ray_direction, self.min, self.max)?;
        const MIN_RAY_DISTANCE: f32 = 0.001;
        let start_distance = entry.max(MIN_RAY_DISTANCE);
        if start_distance > exit {
            return None;
        }

        let start = Vector3::new(
            ray_origin.x + ray_direction.x * start_distance,
            ray_origin.y + ray_direction.y * start_distance,
            ray_origin.z + ray_direction.z * start_distance,
        );
        let mut x = (start.x - self.min.x).floor() as isize;
        let mut y = (start.y - self.min.y).floor() as isize;
        let mut z = (start.z - self.min.z).floor() as isize;
        x = x.clamp(0, self.width as isize - 1);
        y = y.clamp(0, self.height as isize - 1);
        z = z.clamp(0, self.length as isize - 1);

        let (step_x, mut next_x, delta_x) = grid_step(ray_origin.x, ray_direction.x, self.min.x, x);
        let (step_y, mut next_y, delta_y) = grid_step(ray_origin.y, ray_direction.y, self.min.y, y);
        let (step_z, mut next_z, delta_z) = grid_step(ray_origin.z, ray_direction.z, self.min.z, z);
        let mut entered_at = start_distance;
        let steps = [step_x, step_y, step_z];
        let mut entered_axis = if entry > MIN_RAY_DISTANCE {
            entry_axis
        } else {
            None
        };

        while let Some(index) = self.index(x, y, z) {
            let next_boundary = next_x.min(next_y).min(next_z);
            let cell_exit = next_boundary.min(exit);
            if let Some(model_index) = self.blocks[index] {
                let block = &self.models[model_index];
                let cell_min = Vector3::new(
                    self.min.x + x as f32,
                    self.min.y + y as f32,
                    self.min.z + z as f32,
                );

                if let Some(cube) = &block.cube_faces {
                    let (axis, distance, side) = match entered_axis {
                        Some(axis) => (axis, entered_at, if steps[axis] > 0 { 0 } else { 1 }),
                        None => {
                            let axis = if next_x <= next_y && next_x <= next_z {
                                0
                            } else if next_y <= next_z {
                                1
                            } else {
                                2
                            };
                            (axis, next_boundary, if steps[axis] > 0 { 1 } else { 0 })
                        }
                    };
                    let position = Vector3::new(
                        ray_origin.x + ray_direction.x * distance,
                        ray_origin.y + ray_direction.y * distance,
                        ray_origin.z + ray_direction.z * distance,
                    );
                    let mut local = position - cell_min;
                    local.x = local.x.clamp(0.0, 1.0);
                    local.y = local.y.clamp(0.0, 1.0);
                    local.z = local.z.clamp(0.0, 1.0);
                    set_axis_value(&mut local, axis, side as f32);
                    let color = block.faces[cube[axis * 2 + side]].color_at_local(local);

                    let mut normal = Vector3::zero();
                    set_axis_value(&mut normal, axis, -(steps[axis] as f32));
                    return Some(RayHit {
                        material: &block.material,
                        distance,
                        position,
                        normal,
                        color: Some(color),
                    });
                }

                let local_origin = *ray_origin - cell_min;
                let mut closest_hit: Option<RayHit<'_>> = None;
                for face in &block.faces {
                    if let Some((distance, normal, color)) =
                        face.ray_intersect(local_origin, *ray_direction)
                    {
                        if distance + 0.0001 >= entered_at
                            && distance <= cell_exit + 0.0001
                            && closest_hit
                                .as_ref()
                                .is_none_or(|hit| distance < hit.distance)
                        {
                            let position = Vector3::new(
                                ray_origin.x + ray_direction.x * distance,
                                ray_origin.y + ray_direction.y * distance,
                                ray_origin.z + ray_direction.z * distance,
                            );
                            closest_hit = Some(RayHit {
                                material: &block.material,
                                distance,
                                position,
                                normal,
                                color: Some(color),
                            });
                        }
                    }
                }
                for shape in &block.shapes {
                    if let Some((distance, normal)) =
                        shape.ray_intersect(*ray_origin, *ray_direction, cell_min)
                    {
                        if distance + 0.0001 >= entered_at
                            && distance <= cell_exit + 0.0001
                            && closest_hit
                                .as_ref()
                                .is_none_or(|hit| distance < hit.distance)
                        {
                            let position = Vector3::new(
                                ray_origin.x + ray_direction.x * distance,
                                ray_origin.y + ray_direction.y * distance,
                                ray_origin.z + ray_direction.z * distance,
                            );
                            closest_hit = Some(RayHit {
                                material: &block.material,
                                distance,
                                position,
                                normal,
                                color: None,
                            });
                        }
                    }
                }
                if closest_hit.is_some() {
                    return closest_hit;
                }
            }

            if next_x <= next_y && next_x <= next_z {
                entered_at = next_x;
                entered_axis = Some(0);
                x += step_x;
                next_x += delta_x;
            } else if next_y <= next_z {
                entered_at = next_y;
                entered_axis = Some(1);
                y += step_y;
                next_y += delta_y;
            } else {
                entered_at = next_z;
                entered_axis = Some(2);
                z += step_z;
                next_z += delta_z;
            }

            if next_x.min(next_y).min(next_z) > exit + 0.0001 {
                break;
            }
        }

        None
    }
}

impl BlockShape {
    fn ray_intersect(
        &self,
        origin: Vector3,
        direction: Vector3,
        cell_min: Vector3,
    ) -> Option<(f32, Vector3)> {
        match self {
            Self::Box { min, max } => {
                ray_aabb_hit(origin, direction, cell_min + *min, cell_min + *max)
            }
            Self::FlowerCross => {
                let local_origin = origin - cell_min;
                let planes = [
                    (Vector3::new(1.0, 0.0, -1.0), 0.0),
                    (Vector3::new(1.0, 0.0, 1.0), 1.0),
                ];
                planes
                    .into_iter()
                    .filter_map(|(normal, offset)| {
                        let denominator = normal.dot(direction);
                        if denominator.abs() < f32::EPSILON {
                            return None;
                        }
                        let distance = (offset - normal.dot(local_origin)) / denominator;
                        if distance <= 0.001 {
                            return None;
                        }
                        let point = local_origin + direction * distance;
                        if point.x < 0.0
                            || point.x > 1.0
                            || point.y < 0.0
                            || point.y > 1.0
                            || point.z < 0.0
                            || point.z > 1.0
                        {
                            return None;
                        }
                        Some((distance, normal.normalize()))
                    })
                    .min_by(|left, right| left.0.total_cmp(&right.0))
            }
        }
    }
}

fn ray_aabb_hit(
    origin: Vector3,
    direction: Vector3,
    min: Vector3,
    max: Vector3,
) -> Option<(f32, Vector3)> {
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    let mut near_normal = Vector3::zero();
    let mut far_normal = Vector3::zero();
    for (o, d, lo, hi, negative_normal, positive_normal) in [
        (
            origin.x,
            direction.x,
            min.x,
            max.x,
            Vector3::new(-1.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
        ),
        (
            origin.y,
            direction.y,
            min.y,
            max.y,
            Vector3::new(0.0, -1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ),
        (
            origin.z,
            direction.z,
            min.z,
            max.z,
            Vector3::new(0.0, 0.0, -1.0),
            Vector3::new(0.0, 0.0, 1.0),
        ),
    ] {
        if d.abs() < f32::EPSILON {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let mut first = (lo - o) / d;
        let mut second = (hi - o) / d;
        let (mut first_normal, mut second_normal) = (negative_normal, positive_normal);
        if first > second {
            std::mem::swap(&mut first, &mut second);
            std::mem::swap(&mut first_normal, &mut second_normal);
        }
        if first > near {
            near = first;
            near_normal = first_normal;
        }
        if second < far {
            far = second;
            far_normal = second_normal;
        }
        if near > far {
            return None;
        }
    }
    if near > 0.001 {
        Some((near, near_normal))
    } else if far > 0.001 {
        Some((far, far_normal))
    } else {
        None
    }
}

fn grid_step(origin: f32, direction: f32, minimum: f32, cell: isize) -> (isize, f32, f32) {
    if direction > 0.0 {
        let boundary = minimum + cell as f32 + 1.0;
        (1, (boundary - origin) / direction, 1.0 / direction)
    } else if direction < 0.0 {
        let boundary = minimum + cell as f32;
        (-1, (boundary - origin) / direction, -1.0 / direction)
    } else {
        (0, f32::INFINITY, f32::INFINITY)
    }
}

fn ray_box_interval(
    origin: Vector3,
    direction: Vector3,
    min: Vector3,
    max: Vector3,
) -> Option<(f32, f32, Option<usize>)> {
    let mut entry = f32::NEG_INFINITY;
    let mut exit = f32::INFINITY;
    let mut entry_axis = None;

    for (axis, (origin, direction, minimum, maximum)) in [
        (origin.x, direction.x, min.x, max.x),
        (origin.y, direction.y, min.y, max.y),
        (origin.z, direction.z, min.z, max.z),
    ]
    .into_iter()
    .enumerate()
    {
        if direction.abs() < f32::EPSILON {
            if origin < minimum || origin > maximum {
                return None;
            }
            continue;
        }

        let mut near = (minimum - origin) / direction;
        let mut far = (maximum - origin) / direction;
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        if near > entry {
            entry = near;
            entry_axis = Some(axis);
        }
        exit = exit.min(far);
        if entry > exit {
            return None;
        }
    }

    Some((entry, exit, entry_axis))
}

fn material_for_block(block_state: &str, textures: &mut TextureLibrary) -> Option<Material> {
    let block = block_state
        .strip_prefix("minecraft:")
        .unwrap_or(block_state)
        .split('[')
        .next()
        .unwrap_or(block_state);

    if block == "air" || block.ends_with("_air") {
        return None;
    }

    let color = match block {
        "grass_block" | "grass" | "moss_block" | "moss_carpet" | "azalea" => (78, 126, 61),
        "dirt" | "coarse_dirt" | "rooted_dirt" | "dirt_path" => (115, 83, 53),
        "water" => (57, 105, 180),
        "cobblestone" | "stone" | "stone_bricks" | "diorite" | "tuff" => (116, 116, 112),
        "sandstone" | "smooth_sandstone" => (216, 198, 144),
        "quartz_block" => (235, 232, 221),
        "bricks" => (143, 74, 61),
        "hay_block" => (190, 169, 63),
        "gold_block" | "raw_gold_block" => (224, 182, 45),
        "red_carpet" => (170, 40, 35),
        "yellow_carpet" => (226, 190, 46),
        "green_concrete_powder" => (83, 104, 38),
        "light_gray_wool" => (173, 173, 168),
        "gray_wool" => (75, 80, 82),
        "white_wool" => (234, 233, 227),
        "dark_oak_planks" | "dark_oak_wood" | "dark_oak_log" => (65, 43, 29),
        "spruce_planks" | "spruce_log" | "stripped_spruce_wood" => (110, 79, 48),
        "jungle_planks" | "jungle_log" => (151, 110, 74),
        "oak_planks" | "oak_log" | "oak_wood" => (161, 130, 80),
        "birch_planks" => (197, 188, 145),
        "mushroom_stem" => (202, 193, 168),
        "iron_bars" | "iron_trapdoor" => (154, 162, 166),
        "lantern" | "candle" | "torch" | "wall_torch" => (246, 196, 79),
        "flower_pot" => (136, 75, 48),
        "azure_bluet" | "dandelion" | "lily_of_the_valley" => (237, 225, 135),
        block if block.contains("leaves") => (50, 111, 48),
        block if block.contains("glass") => (156, 202, 213),
        block if block.contains("planks") || block.contains("log") || block.contains("wood") => {
            (133, 94, 56)
        }
        block
            if block.contains("fence") || block.contains("trapdoor") || block.contains("door") =>
        {
            (121, 83, 47)
        }
        block if block.contains("stairs") || block.contains("slab") => (117, 86, 55),
        _ => (150, 150, 150),
    };

    Some(textures.material_for(block, Color::new(color.0, color.1, color.2, 255)))
}

fn shapes_for_block(block_state: &str) -> Vec<BlockShape> {
    let block = block_state
        .strip_prefix("minecraft:")
        .unwrap_or(block_state)
        .split('[')
        .next()
        .unwrap_or(block_state);
    let full = || vec![box_shape(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)];

    match block {
        "lantern" => {
            let hanging = state_value(block_state, "hanging") == Some("true");
            let body_bottom = if hanging { 0.0 } else { 0.0 };
            let mut shapes = vec![
                box_shape(
                    5.0 / 16.0,
                    body_bottom,
                    5.0 / 16.0,
                    11.0 / 16.0,
                    7.0 / 16.0,
                    11.0 / 16.0,
                ),
                box_shape(
                    6.0 / 16.0,
                    7.0 / 16.0,
                    6.0 / 16.0,
                    10.0 / 16.0,
                    9.0 / 16.0,
                    10.0 / 16.0,
                ),
            ];
            if hanging {
                shapes.push(box_shape(
                    7.0 / 16.0,
                    9.0 / 16.0,
                    7.0 / 16.0,
                    9.0 / 16.0,
                    1.0,
                    9.0 / 16.0,
                ));
            } else {
                shapes.push(box_shape(
                    7.0 / 16.0,
                    9.0 / 16.0,
                    7.0 / 16.0,
                    9.0 / 16.0,
                    11.0 / 16.0,
                    9.0 / 16.0,
                ));
            }
            shapes
        }
        "azure_bluet" | "dandelion" | "lily_of_the_valley" | "grass" => {
            vec![BlockShape::FlowerCross]
        }
        "iron_bars" => {
            let mut shapes = vec![box_shape(
                7.0 / 16.0,
                0.0,
                7.0 / 16.0,
                9.0 / 16.0,
                1.0,
                9.0 / 16.0,
            )];
            if state_value(block_state, "north") == Some("true") {
                shapes.push(box_shape(7.0 / 16.0, 0.0, 0.0, 9.0 / 16.0, 1.0, 0.5));
            }
            if state_value(block_state, "south") == Some("true") {
                shapes.push(box_shape(7.0 / 16.0, 0.0, 0.5, 9.0 / 16.0, 1.0, 1.0));
            }
            if state_value(block_state, "west") == Some("true") {
                shapes.push(box_shape(0.0, 0.0, 7.0 / 16.0, 0.5, 1.0, 9.0 / 16.0));
            }
            if state_value(block_state, "east") == Some("true") {
                shapes.push(box_shape(0.5, 0.0, 7.0 / 16.0, 1.0, 1.0, 9.0 / 16.0));
            }
            shapes
        }
        block if block.ends_with("_trapdoor") => {
            let thickness = 3.0 / 16.0;
            if state_value(block_state, "open") == Some("true") {
                match state_value(block_state, "facing") {
                    Some("north") => vec![box_shape(0.0, 0.0, 0.0, 1.0, 1.0, thickness)],
                    Some("south") => vec![box_shape(0.0, 0.0, 1.0 - thickness, 1.0, 1.0, 1.0)],
                    Some("west") => vec![box_shape(0.0, 0.0, 0.0, thickness, 1.0, 1.0)],
                    Some("east") => vec![box_shape(1.0 - thickness, 0.0, 0.0, 1.0, 1.0, 1.0)],
                    _ => full(),
                }
            } else if state_value(block_state, "half") == Some("top") {
                vec![box_shape(0.0, 1.0 - thickness, 0.0, 1.0, 1.0, 1.0)]
            } else {
                vec![box_shape(0.0, 0.0, 0.0, 1.0, thickness, 1.0)]
            }
        }
        block if block.ends_with("_door") => {
            let thickness = 3.0 / 16.0;
            match state_value(block_state, "facing") {
                Some("north") => vec![box_shape(0.0, 0.0, 0.0, 1.0, 1.0, thickness)],
                Some("south") => vec![box_shape(0.0, 0.0, 1.0 - thickness, 1.0, 1.0, 1.0)],
                Some("west") => vec![box_shape(0.0, 0.0, 0.0, thickness, 1.0, 1.0)],
                Some("east") => vec![box_shape(1.0 - thickness, 0.0, 0.0, 1.0, 1.0, 1.0)],
                _ => full(),
            }
        }
        "candle" => vec![box_shape(
            7.0 / 16.0,
            0.0,
            7.0 / 16.0,
            9.0 / 16.0,
            7.0 / 16.0,
            9.0 / 16.0,
        )],
        "wall_torch" => match state_value(block_state, "facing") {
            Some("north") => vec![box_shape(
                6.0 / 16.0,
                3.0 / 16.0,
                8.0 / 16.0,
                10.0 / 16.0,
                13.0 / 16.0,
                1.0,
            )],
            Some("south") => vec![box_shape(
                6.0 / 16.0,
                3.0 / 16.0,
                0.0,
                10.0 / 16.0,
                13.0 / 16.0,
                8.0 / 16.0,
            )],
            Some("west") => vec![box_shape(
                8.0 / 16.0,
                3.0 / 16.0,
                6.0 / 16.0,
                1.0,
                13.0 / 16.0,
                10.0 / 16.0,
            )],
            Some("east") => vec![box_shape(
                0.0,
                3.0 / 16.0,
                6.0 / 16.0,
                8.0 / 16.0,
                13.0 / 16.0,
                10.0 / 16.0,
            )],
            _ => full(),
        },
        _ => full(),
    }
}

fn box_shape(min_x: f32, min_y: f32, min_z: f32, max_x: f32, max_y: f32, max_z: f32) -> BlockShape {
    BlockShape::Box {
        min: Vector3::new(min_x, min_y, min_z),
        max: Vector3::new(max_x, max_y, max_z),
    }
}

fn state_value<'a>(block_state: &'a str, name: &str) -> Option<&'a str> {
    let states = block_state.split_once('[')?.1.strip_suffix(']')?;
    states.split(',').find_map(|state| {
        let (key, value) = state.split_once('=')?;
        (key == name).then_some(value)
    })
}

fn decode_varints(data: &[u8]) -> Result<Vec<i32>, String> {
    let mut values = Vec::new();
    let mut position = 0;

    while position < data.len() {
        let mut value = 0_i32;
        let mut shift = 0;
        loop {
            let byte = *data.get(position).ok_or("truncated BlockData varint")?;
            position += 1;
            value |= i32::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 35 {
                return Err("BlockData contains an oversized varint".into());
            }
        }
        values.push(value);
    }

    Ok(values)
}

struct SchematicNbt {
    width: usize,
    height: usize,
    length: usize,
    palette: HashMap<i32, String>,
    block_data: Vec<u8>,
}

impl SchematicNbt {
    fn parse(data: &[u8]) -> Result<Self, String> {
        let mut reader = NbtReader { data, position: 0 };
        if reader.read_u8()? != 10 {
            return Err("schematic root is not an NBT compound".into());
        }
        reader.read_string()?;

        let mut width = None;
        let mut height = None;
        let mut length = None;
        let mut palette = HashMap::new();
        let mut block_data = None;

        loop {
            let tag = reader.read_u8()?;
            if tag == 0 {
                break;
            }
            let name = reader.read_string()?;
            match (tag, name.as_str()) {
                (2, "Width") => width = Some(reader.read_i16()? as usize),
                (2, "Height") => height = Some(reader.read_i16()? as usize),
                (2, "Length") => length = Some(reader.read_i16()? as usize),
                (7, "BlockData") => block_data = Some(reader.read_byte_array()?),
                (10, "Palette") => palette = reader.read_palette()?,
                _ => reader.skip_payload(tag)?,
            }
        }

        Ok(Self {
            width: width.ok_or("schematic is missing Width")?,
            height: height.ok_or("schematic is missing Height")?,
            length: length.ok_or("schematic is missing Length")?,
            palette,
            block_data: block_data.ok_or("schematic is missing BlockData")?,
        })
    }
}

struct NbtReader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> NbtReader<'a> {
    fn read_u8(&mut self) -> Result<u8, String> {
        let value = *self
            .data
            .get(self.position)
            .ok_or("unexpected end of NBT")?;
        self.position += 1;
        Ok(value)
    }

    fn read_i16(&mut self) -> Result<i16, String> {
        let bytes = self.read_exact(2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_i32(&mut self) -> Result<i32, String> {
        let bytes = self.read_exact(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_string(&mut self) -> Result<String, String> {
        let length = u16::from_be_bytes(self.read_exact(2)?.try_into().unwrap()) as usize;
        let bytes = self.read_exact(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| "invalid UTF-8 in NBT string".into())
    }

    fn read_byte_array(&mut self) -> Result<Vec<u8>, String> {
        let length = self.read_i32()?;
        if length < 0 {
            return Err("negative NBT byte-array length".into());
        }
        Ok(self.read_exact(length as usize)?.to_vec())
    }

    fn read_palette(&mut self) -> Result<HashMap<i32, String>, String> {
        let mut palette = HashMap::new();
        loop {
            let tag = self.read_u8()?;
            if tag == 0 {
                break;
            }
            let block_state = self.read_string()?;
            if tag != 3 {
                return Err("schematic Palette has a non-integer value".into());
            }
            palette.insert(self.read_i32()?, block_state);
        }
        Ok(palette)
    }

    fn read_exact(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .position
            .checked_add(length)
            .ok_or("NBT length overflow")?;
        let bytes = self
            .data
            .get(self.position..end)
            .ok_or("unexpected end of NBT")?;
        self.position = end;
        Ok(bytes)
    }

    fn skip_payload(&mut self, tag: u8) -> Result<(), String> {
        match tag {
            1 => self.skip(1),
            2 => self.skip(2),
            3 | 5 => self.skip(4),
            4 | 6 => self.skip(8),
            7 => {
                let length = self.read_i32()?;
                self.skip(checked_length(length)?)
            }
            8 => {
                let length = u16::from_be_bytes(self.read_exact(2)?.try_into().unwrap()) as usize;
                self.skip(length)
            }
            9 => {
                let element_tag = self.read_u8()?;
                let length = checked_length(self.read_i32()?)?;
                for _ in 0..length {
                    self.skip_payload(element_tag)?;
                }
                Ok(())
            }
            10 => loop {
                let nested_tag = self.read_u8()?;
                if nested_tag == 0 {
                    return Ok(());
                }
                self.read_string()?;
                self.skip_payload(nested_tag)?;
            },
            11 => {
                let length = checked_length(self.read_i32()?)?;
                self.skip(length.checked_mul(4).ok_or("NBT array too large")?)
            }
            12 => {
                let length = checked_length(self.read_i32()?)?;
                self.skip(length.checked_mul(8).ok_or("NBT array too large")?)
            }
            _ => Err(format!("unknown NBT tag type {tag}")),
        }
    }

    fn skip(&mut self, length: usize) -> Result<(), String> {
        self.read_exact(length).map(|_| ())
    }
}

fn checked_length(length: i32) -> Result<usize, String> {
    usize::try_from(length).map_err(|_| "negative NBT collection length".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_the_bundled_lon_lon_ranch_schematic() {
        let world = VoxelWorld::from_schematic(include_bytes!("../assets/lonlonranchclean.schem"))
            .expect("the bundled schematic should load");

        assert!(world.width > 0 && world.height > 0 && world.length > 0);
        assert!(world.blocks.iter().any(Option::is_some));
    }
}
