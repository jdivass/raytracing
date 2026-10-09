use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use raylib::prelude::*;

#[derive(Clone)]
pub struct Material {
    pub diffuse: Color,
    textures: Option<Arc<BlockTextures>>,
}

#[derive(Clone)]
pub(crate) struct BlockTextures {
    side: Arc<Texture>,
    top: Arc<Texture>,
    bottom: Arc<Texture>,
}

pub(crate) struct Texture {
    width: i32,
    height: i32,
    pixels: Vec<Color>,
}

pub struct TextureLibrary {
    directory: PathBuf,
    cache: HashMap<String, Option<Arc<Texture>>>,
}

impl Material {
    pub fn solid(diffuse: Color) -> Self {
        Self {
            diffuse,
            textures: None,
        }
    }

    fn textured(diffuse: Color, textures: BlockTextures) -> Self {
        Self {
            diffuse,
            textures: Some(Arc::new(textures)),
        }
    }

    pub fn color_at(&self, position: Vector3, normal: Vector3) -> Color {
        let Some(textures) = &self.textures else {
            return self.diffuse;
        };

        let (texture, u, v) = if normal.y > 0.5 {
            (&textures.top, position.x, position.z)
        } else if normal.y < -0.5 {
            (&textures.bottom, position.x, position.z)
        } else if normal.x.abs() > 0.5 {
            (&textures.side, position.z, position.y)
        } else {
            (&textures.side, position.x, position.y)
        };

        texture.sample(u, v)
    }
}

impl TextureLibrary {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            cache: HashMap::new(),
        }
    }

    pub fn material_for(&mut self, block: &str, fallback: Color) -> Material {
        let (side_name, top_name, bottom_name) = texture_names_for(block);
        let side_variant = format!("{side_name}_side");
        let side = self.first_existing(&[&side_name, &side_variant]);
        let Some(side) = side else {
            return Material::solid(fallback);
        };
        let top = self
            .first_existing(&[&top_name, &format!("{top_name}_top")])
            .unwrap_or_else(|| side.clone());
        let bottom = self
            .first_existing(&[&bottom_name, &format!("{bottom_name}_bottom")])
            .unwrap_or_else(|| side.clone());

        Material::textured(fallback, BlockTextures { side, top, bottom })
    }

    fn first_existing(&mut self, names: &[&str]) -> Option<Arc<Texture>> {
        names.iter().find_map(|name| self.load(name))
    }

    fn load(&mut self, name: &str) -> Option<Arc<Texture>> {
        let resource = name.strip_prefix("minecraft:").unwrap_or(name);
        let resource = if resource.contains('/') {
            resource.to_owned()
        } else {
            format!("block/{resource}")
        };
        if let Some(texture) = self.cache.get(&resource) {
            return texture.clone();
        }

        let path = self.directory.join(format!("{resource}.png"));
        if !path.is_file() {
            self.cache.insert(resource, None);
            return None;
        }
        let texture = load_texture(&path);
        self.cache.insert(resource, texture.clone());
        texture
    }

    pub(crate) fn load_model_texture(&mut self, name: &str) -> Option<Arc<Texture>> {
        self.load(name)
    }
}

impl Texture {
    pub(crate) fn sample_uv(&self, u: f32, v: f32) -> Color {
        let u = u.rem_euclid(16.0).min(15.9999);
        let v = v.rem_euclid(16.0).min(15.9999);
        let x = (u / 16.0 * self.width as f32).floor() as i32;
        let y = (v / 16.0 * self.height as f32).floor() as i32;
        self.pixels
            [(y.clamp(0, self.height - 1) * self.width + x.clamp(0, self.width - 1)) as usize]
    }

    fn sample(&self, u: f32, v: f32) -> Color {
        let u = u.rem_euclid(1.0);
        let v = v.rem_euclid(1.0);
        let x = (u * self.width as f32).floor() as i32;
        let y = ((1.0 - v.rem_euclid(1.0)) * self.height as f32).floor() as i32;
        self.pixels
            [(y.clamp(0, self.height - 1) * self.width + x.clamp(0, self.width - 1)) as usize]
    }
}

fn load_texture(path: &Path) -> Option<Arc<Texture>> {
    let image = Image::load_image(path.to_str()?).ok()?;
    let width = image.width();
    let height = image.height();
    if width <= 0 || height <= 0 {
        return None;
    }

    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.push(image.get_color(x, y));
        }
    }
    Some(Arc::new(Texture {
        width,
        height,
        pixels,
    }))
}

fn texture_names_for(block: &str) -> (String, String, String) {
    match block {
        "grass_block" => (
            "grass_block_side".into(),
            "grass_block_top".into(),
            "dirt".into(),
        ),
        "dirt_path" => (
            "dirt_path_side".into(),
            "dirt_path_top".into(),
            "dirt_path_side".into(),
        ),
        "barrel" => (
            "barrel_side".into(),
            "barrel_top".into(),
            "barrel_bottom".into(),
        ),
        "hay_block" => (
            "hay_block_side".into(),
            "hay_block_top".into(),
            "hay_block_side".into(),
        ),
        "quartz_block" => (
            "quartz_block_side".into(),
            "quartz_block_top".into(),
            "quartz_block_bottom".into(),
        ),
        "chiseled_quartz_block" => (
            "chiseled_quartz_block".into(),
            "chiseled_quartz_block_top".into(),
            "chiseled_quartz_block".into(),
        ),
        "water" => (
            "water_still".into(),
            "water_still".into(),
            "water_still".into(),
        ),
        "wall_torch" => ("torch".into(), "torch".into(), "torch".into()),
        "azalea" => (
            "azalea_plant".into(),
            "azalea_plant".into(),
            "azalea_plant".into(),
        ),
        "grass" => (
            "short_grass".into(),
            "short_grass".into(),
            "short_grass".into(),
        ),
        "composter" => (
            "composter_side".into(),
            "composter_top".into(),
            "composter_bottom".into(),
        ),
        "moss_carpet" => (
            "moss_block".into(),
            "moss_block".into(),
            "moss_block".into(),
        ),
        "red_carpet" => ("red_wool".into(), "red_wool".into(), "red_wool".into()),
        "yellow_carpet" => (
            "yellow_wool".into(),
            "yellow_wool".into(),
            "yellow_wool".into(),
        ),
        "sign" => (
            "oak_planks".into(),
            "oak_planks".into(),
            "oak_planks".into(),
        ),
        "mushroom_stem" => (
            "mushroom_stem".into(),
            "mushroom_stem".into(),
            "mushroom_stem".into(),
        ),
        block if block.ends_with("_stained_glass_pane") => {
            let glass = block.trim_end_matches("_pane");
            (glass.into(), format!("{block}_top"), format!("{block}_top"))
        }
        block if block.ends_with("_leaves") => (block.into(), block.into(), block.into()),
        block if block.ends_with("_door") => {
            let stem = block.trim_end_matches("_door");
            (
                format!("{stem}_door_bottom"),
                format!("{stem}_door_top"),
                format!("{stem}_door_bottom"),
            )
        }
        block if block.ends_with("_wood") => {
            let log = block.replacen("_wood", "_log", 1);
            (log.clone(), format!("{log}_top"), format!("{log}_top"))
        }
        block if block.ends_with("_log") => (
            block.to_owned(),
            format!("{block}_top"),
            format!("{block}_top"),
        ),
        block if block.ends_with("_stairs") => plank_texture(block.trim_end_matches("_stairs")),
        block if block.ends_with("_slab") => plank_texture(block.trim_end_matches("_slab")),
        block if block.ends_with("_fence_gate") => {
            plank_texture(block.trim_end_matches("_fence_gate"))
        }
        block if block.ends_with("_fence") => plank_texture(block.trim_end_matches("_fence")),
        block if block.ends_with("_wall_sign") => {
            plank_texture(block.trim_end_matches("_wall_sign"))
        }
        block => (
            block.into(),
            format!("{block}_top"),
            format!("{block}_bottom"),
        ),
    }
}

fn plank_texture(base: &str) -> (String, String, String) {
    match base {
        "quartz" => (
            "quartz_block_side".into(),
            "quartz_block_top".into(),
            "quartz_block_bottom".into(),
        ),
        "stone_brick" => (
            "stone_bricks".into(),
            "stone_bricks".into(),
            "stone_bricks".into(),
        ),
        "oak" | "spruce" | "birch" | "jungle" | "dark_oak" | "acacia" | "bamboo" => {
            let planks = format!("{base}_planks");
            (planks.clone(), planks.clone(), planks)
        }
        _ => (base.into(), base.into(), base.into()),
    }
}
