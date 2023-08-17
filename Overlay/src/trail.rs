use std::{
    error::Error,
    fs,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    str::FromStr,
    sync::{Arc, RwLock},
};

use bevy::{
    prelude::{Mesh, Vec2, Vec3, Vec4},
    render::{mesh::Indices, render_resource::PrimitiveTopology},
};
use log::error;

use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Deserializer};

use crate::gw2poi::POI;
use crate::utils::ToGw2Coordinate;

pub type TrailContainer = Arc<RwLock<Trail>>;

pub fn deserialize_trail_vec<'de, D>(deserializer: D) -> Result<Vec<TrailContainer>, D::Error>
where
    D: Deserializer<'de>,
{
    let trail = Vec::<Trail>::deserialize(deserializer)?;
    let new_vec: Vec<TrailContainer> = trail
        .iter()
        .map(|p| Arc::new(RwLock::new(p.clone())))
        .collect();

    Ok(new_vec)
}

#[derive(Debug, Default, Clone)]
struct TrailData {
    x: f32,
    y: f32,
    z: f32,
}

impl From<Vec3> for TrailData {
    fn from(value: Vec3) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
        }
    }
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct Trail {
    #[serde(rename = "trailData")]
    pub trail_file: PathBuf,
    pub texture: PathBuf,
    pub color: Option<String>,
    #[serde(rename = "animSpeed")]
    pub anim_speed: f32,
    #[serde(flatten)]
    pub poi: POI,

    #[serde(skip)]
    trail_data: Vec<TrailData>,
}

impl Trail {
    pub fn load_map_trail(&mut self) -> Result<(), Box<dyn Error>> {
        // TODO: get from asset server
        let mut file_path = PathBuf::from_str("Overlay/assets").unwrap();
        file_path.push(self.trail_file.clone());
        let f = fs::File::open(file_path);
        match f {
            Ok(mut file) => {
                let total_len = file.metadata()?.len();
                if total_len >= 8 {
                    file.seek(SeekFrom::Start(4))?;
                    let mut buffer = [0u8; 4];
                    file.read_exact(&mut buffer)?;
                    let map_id = u32::from_le_bytes(buffer);
                    self.poi.set_map_id(Some(map_id));

                    // Calculate the number of coordinates in the file
                    let coord_size = std::mem::size_of::<TrailData>();
                    let mut buffer = Vec::new();
                    file.read_to_end(&mut buffer)?;
                    let num_coords = buffer.len() / coord_size;

                    // Read data from the buffer into the vector of structs
                    for i in 0..num_coords {
                        let offset = i * coord_size;
                        let mut cursor = std::io::Cursor::new(&buffer[offset..offset + coord_size]);

                        let x = cursor.read_f32::<LittleEndian>()?;
                        let y = cursor.read_f32::<LittleEndian>()?;
                        let z = cursor.read_f32::<LittleEndian>()?;
                        let pos = Vec3::new(x, y, z);

                        #[cfg(not(feature = "custom_projection"))]
                        let trail = TrailData::from(pos.as_gw2_coordinate());
                        #[cfg(feature = "custom_projection")]
                        let trail = TrailData::from(pos);
                        self.trail_data.push(trail);
                    }
                }
            }
            Err(e) => error!("Failed to load trail data: {}", e),
        }
        Ok(())
    }

    fn get_perpendicular_point(p1: Vec3, p2: Vec3, distance: f32) -> (Vec3, Vec3) {
        let mut a = p1.z - p2.z;
        let mut b = p1.x - p2.x;

        let norm = f32::sqrt(a * a + b * b);
        a = a / norm;
        b /= norm;

        let mut out1 = Vec3::ZERO;
        let mut out2 = Vec3::ZERO;

        out1.x = p2.x - a * distance;
        out1.z = p2.z + b * distance;
        out1.y = p2.y;

        out2.x = p2.x + a * distance;
        out2.z = p2.z - b * distance;
        out2.y = p2.y;

        (out1, out2)
    }

    pub fn generate_meshes(&self) -> Vec<Mesh> {
        let mut meshes = vec![];
        let mut vertices = vec![];
        let mut indices = vec![];
        let width = 0.5;
        let mut current_index = 0;

        let mut prev_data: Option<Vec3> = None;
        let mut prev_p1 = Vec3::ZERO;
        let mut prev_p2 = Vec3::ZERO;
        self.trail_data.iter().for_each(|trail| {
            let current_data = Vec3::new(trail.x, trail.y, trail.z);
            if current_data.x as i32 == 0
                && current_data.y as i32 == 0
                && current_data.z as i32 == 0
            {
                if vertices.len() > 0 && indices.len() > 0 {
                    let mesh = create_mesh(vertices.clone(), indices.clone());
                    meshes.push(mesh);
                    vertices.clear();
                    indices.clear();
                    current_index = 0;
                    prev_data = None;
                }
                return (); // continue
            }
            match prev_data {
                Some(prev_data) => {
                    vertices.push(Vertex::new(prev_p1, Vec4::ONE, Vec2::new(0.0, 0.0)));
                    vertices.push(Vertex::new(prev_p2, Vec4::ONE, Vec2::new(1.0, 0.0)));
                    (prev_p1, prev_p2) =
                        Trail::get_perpendicular_point(prev_data, current_data, width);
                    // Calculate distance between the last and current point to adjust the uv
                    // coordinates
                    let distance = prev_data.distance(current_data);
                    // TODO: Fix very long trail segments
                    // Negative to flip the direction
                    let frac = 1.0f32.max(distance / width) * -1.0;
                    vertices.push(Vertex::new(prev_p2, Vec4::ONE, Vec2::new(1.0, frac)));
                    vertices.push(Vertex::new(prev_p1, Vec4::ONE, Vec2::new(0.0, frac)));
                    indices.push(current_index);
                    indices.push(current_index + 1);
                    indices.push(current_index + 2);
                    indices.push(current_index + 2);
                    indices.push(current_index + 3);
                    indices.push(current_index);
                    current_index += 4;
                }
                None => {
                    // Set initial starting points from where to build the trail mesh
                    prev_p1 = Vec3::from_array([trail.x - width, trail.y, trail.z]);
                    prev_p2 = Vec3::from_array([trail.x + width, trail.y, trail.z]);
                }
            }
            prev_data = Some(current_data);
        });
        if vertices.len() > 0 && indices.len() > 0 {
            let mesh = create_mesh(vertices, indices);
            meshes.push(mesh);
        }
        meshes
    }
}

fn create_mesh(vertices: Vec<Vertex>, indices: Vec<u32>) -> Mesh {
    let mut cube_mesh = Mesh::new(PrimitiveTopology::TriangleList);
    cube_mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vertices.iter().map(|v| v.pos).collect::<Vec<Vec3>>(),
    );
    cube_mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vertices.iter().map(|v| v.color).collect::<Vec<Vec4>>(),
    );
    cube_mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vertices.iter().map(|v| v.tex_coord).collect::<Vec<Vec2>>(),
    );

    cube_mesh.set_indices(Some(Indices::U32(indices)));
    cube_mesh
}

#[derive(Debug, Default, Clone, Copy)]
struct Vertex {
    pos: Vec3,
    color: Vec4,
    tex_coord: Vec2,
}

impl Vertex {
    pub fn new(pos: Vec3, color: Vec4, tex_coord: Vec2) -> Self {
        Self {
            pos,
            color,
            tex_coord,
        }
    }
}
