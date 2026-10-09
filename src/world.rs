use std::collections::HashMap;
use std::io::Read;

use flate2::read::GzDecoder;
use raylib::prelude::*;

use crate::material::Material;
use crate::ray_intersect::RayIntersect;

pub struct VoxelWorld {
    width: usize,
    height: usize,
    length: usize,
    blocks: Vec<Option<Material>>,
    min: Vector3,
    max: Vector3,
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
        let palette = schematic
            .palette
            .into_iter()
            .map(|(index, block)| (index, material_for_block(&block)))
            .collect::<HashMap<_, _>>();

        let palette_indices = decode_varints(&schematic.block_data)?;
        if palette_indices.len() < block_count {
            return Err(format!(
                "schematic contains {} blocks, expected {block_count}",
                palette_indices.len()
            ));
        }

        // Sponge schematics store blocks with X changing first, then Z, then Y.
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
            min,
            max,
        })
    }

    pub fn camera_start(&self) -> Vector3 {
        let largest_dimension = self.width.max(self.length) as f32;
        Vector3::new(
            0.0,
            self.height as f32 + largest_dimension * 0.9,
            self.max.z + largest_dimension * 0.65,
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

    fn block_at(&self, x: isize, y: isize, z: isize) -> Option<Material> {
        self.index(x, y, z).and_then(|index| self.blocks[index])
    }
}

impl RayIntersect for VoxelWorld {
    fn ray_intersect(
        &self,
        ray_origin: &Vector3,
        ray_direction: &Vector3,
    ) -> Option<(Material, f32)> {
        let (entry, exit) = ray_box_interval(*ray_origin, *ray_direction, self.min, self.max)?;
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

        while let Some(index) = self.index(x, y, z) {
            if let Some(material) = self.blocks[index] {
                let distance = next_x.min(next_y).min(next_z).min(exit).max(start_distance);
                return Some((material, distance));
            }

            if next_x <= next_y && next_x <= next_z {
                x += step_x;
                next_x += delta_x;
            } else if next_y <= next_z {
                y += step_y;
                next_y += delta_y;
            } else {
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
) -> Option<(f32, f32)> {
    let mut entry = f32::NEG_INFINITY;
    let mut exit = f32::INFINITY;

    for (origin, direction, minimum, maximum) in [
        (origin.x, direction.x, min.x, max.x),
        (origin.y, direction.y, min.y, max.y),
        (origin.z, direction.z, min.z, max.z),
    ] {
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
        entry = entry.max(near);
        exit = exit.min(far);
        if entry > exit {
            return None;
        }
    }

    Some((entry, exit))
}

fn material_for_block(block_state: &str) -> Option<Material> {
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

    Some(Material {
        diffuse: Color::new(color.0, color.1, color.2, 255),
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
        let world = VoxelWorld::from_schematic(include_bytes!("../assets/lonlonranch.schem"))
            .expect("the bundled schematic should load");

        assert!(world.width > 0 && world.height > 0 && world.length > 0);
        assert!(world.blocks.iter().any(Option::is_some));
    }
}
