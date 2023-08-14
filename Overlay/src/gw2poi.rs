use std::{
    collections::HashMap,
    error::Error,
    fmt::Display,
    fs,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    str::FromStr,
    sync::{Arc, RwLock},
};

use bevy::{
    prelude::{info, Mesh, Vec2, Vec3},
    render::{mesh::Indices, render_resource::PrimitiveTopology},
};
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Deserializer, Serialize};

use log::error;

pub type MarkerCategoryContainer = Arc<RwLock<MarkerCategory>>;
pub type PoiContainer = Arc<RwLock<POI>>;
pub type TrailContainer = Arc<RwLock<Trail>>;

fn deserialize_option_path<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    let p = Option::<String>::deserialize(deserializer)?;
    let p = match p {
        Some(p) => Some(PathBuf::from(p.replace(r"\", "/"))),
        None => None,
    };

    Ok(p)
}

fn deserialize_marker_category_hashmap<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, MarkerCategoryContainer>, D::Error>
where
    D: Deserializer<'de>,
{
    let cat = Vec::<MarkerCategory>::deserialize(deserializer)?;
    let map: HashMap<String, MarkerCategory> =
        cat.iter().map(|c| (c.name.clone(), c.clone())).collect();

    let arc_map: HashMap<String, MarkerCategoryContainer> = map
        .iter()
        .map(|(key, val)| (key.clone(), Arc::new(RwLock::new(val.clone()))))
        .collect();

    Ok(arc_map)
}

fn deserialize_marker_category_vec<'de, D>(
    deserializer: D,
) -> Result<Vec<MarkerCategoryContainer>, D::Error>
where
    D: Deserializer<'de>,
{
    let cat = Vec::<MarkerCategory>::deserialize(deserializer)?;
    let new_vec = cat
        .iter()
        .map(|c| Arc::new(RwLock::new(c.clone())))
        .collect();

    Ok(new_vec)
}

fn deserialize_poi_vec<'de, D>(deserializer: D) -> Result<Vec<PoiContainer>, D::Error>
where
    D: Deserializer<'de>,
{
    let poi = Vec::<POI>::deserialize(deserializer);
    if poi.is_err() {
        return Ok(vec![]);
    }
    let poi = poi.unwrap();
    let new_vec: Vec<PoiContainer> = poi
        .iter()
        .map(|p| Arc::new(RwLock::new(p.clone())))
        .collect();

    Ok(new_vec)
}

fn deserialize_trail_vec<'de, D>(deserializer: D) -> Result<Vec<TrailContainer>, D::Error>
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

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MarkerCategory {
    pub name: String,
    #[serde(
        default,
        rename = "MarkerCategory",
        deserialize_with = "deserialize_marker_category_hashmap"
    )]
    pub children: HashMap<String, MarkerCategoryContainer>,
    //children: Option<Vec<MarkerCategory>>,
    #[serde(flatten)]
    pub data: POI,
}

impl MarkerCategory {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
    /// XXX: only searches children!
    pub fn get_category_children(&self, name: &str) -> Option<MarkerCategoryContainer> {
        let next_name = name.split_once('.');
        let pass_name;
        let child_name;
        match next_name {
            Some(next_name) => {
                child_name = next_name.0;
                pass_name = next_name.1;
            }
            None => {
                child_name = name;
                pass_name = "";
            }
        }

        let child = self.children.get(child_name);
        match child {
            Some(child) => {
                let child_name = child.read().unwrap().name.clone();
                if name == child_name {
                    return Some(child.clone());
                } else {
                    return child.read().unwrap().get_category_children(pass_name);
                }
            }
            None => (),
        }
        return None;
    }
}

pub trait PoiTrait {
    fn get_parent_category(&self) -> Option<MarkerCategoryContainer>;
    fn set_parent(&mut self, parent: Option<MarkerCategoryContainer>);

    fn get_parent(&self) -> Option<MarkerCategoryContainer>;
}

impl PoiTrait for MarkerCategory {
    fn get_parent_category(&self) -> Option<MarkerCategoryContainer> {
        self.data.get_parent_category()
    }

    fn set_parent(&mut self, parent: Option<MarkerCategoryContainer>) {
        self.data.set_parent(parent);
    }

    fn get_parent(&self) -> Option<MarkerCategoryContainer> {
        self.data.get_parent()
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OverlayData {
    #[serde(
        rename = "MarkerCategory",
        deserialize_with = "deserialize_marker_category_vec"
    )]
    pub marker_category: Vec<MarkerCategoryContainer>,
    #[serde(rename = "POIs")]
    pub pois: POIs,
}

impl OverlayData {
    pub fn merge(&mut self, mut other: OverlayData) {
        self.pois.poi_list.append(&mut other.pois.poi_list);
        self.pois.trail_list.append(&mut other.pois.trail_list);
        self.marker_category.append(&mut other.marker_category);
    }
    pub fn from_file(file_path: &str) -> Result<Self, Box<dyn Error>> {
        let file_handle = fs::File::open(file_path).unwrap();

        let mut de = serde_xml_rs::Deserializer::new_from_reader(file_handle)
            .non_contiguous_seq_elements(true);
        let data = OverlayData::deserialize(&mut de)?;
        info!(
            "Loaded {} POIs and {} Trails",
            data.pois.poi_list.len(),
            data.pois.trail_list.len()
        );
        Ok(data)
    }

    pub fn from_string(data: &str) -> Self {
        let mut de = serde_xml_rs::Deserializer::new_from_reader(data.as_bytes())
            .non_contiguous_seq_elements(true);
        OverlayData::deserialize(&mut de).unwrap()
    }

    pub fn fill_poi_parents(&mut self) {
        self.pois.trail_list.iter().for_each(|trail_lock| {
            let mut trail = trail_lock.write().unwrap();
            info!("Filling trail {:?}", trail.texture);
            trail.load_map_trail().unwrap();
        });
        self.pois.poi_list.iter_mut().for_each(|poi| {
            self.marker_category.iter().for_each(|category| {
                let category_name = poi.read().unwrap().poi_type.clone();
                // TODO: I hate it! Check if the whole name is in marker_category. If not pass the seond
                // part to get_category_children
                match category_name {
                    Some(name) => {
                        let (first_name, second_name) = name.split_once('.').unwrap_or((&name, ""));
                        if category.read().unwrap().name == name {
                            poi.write().unwrap().set_parent(Some(category.clone()));
                        } else if category.read().unwrap().name == first_name {
                            let found_category =
                                category.read().unwrap().get_category_children(second_name);
                            match found_category {
                                Some(cat) => {
                                    poi.write().unwrap().set_parent(Some(cat));
                                }
                                None => (),
                            }
                        }
                    }
                    None => (),
                }
            });
        });
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct POIs {
    #[serde(rename = "POI", deserialize_with = "deserialize_poi_vec")]
    pub poi_list: Vec<PoiContainer>,
    #[serde(rename = "Trail", deserialize_with = "deserialize_trail_vec")]
    pub trail_list: Vec<TrailContainer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoiBehavior {
    DEFAULT = 0,
    REAPPEAR_ON_MAP_CHANGE = 1,
    REAPPEAR_ON_DAILY_RESET = 2,
    ONLY_VISIBLE_BEFORE_ACTIVATION = 3,
    REAPPEAR_AFTER_TIMER = 4,
    REAPPEAR_ON_MAP_RESET = 5,
    ONCE_PER_INSTANCE = 6,
    ONCE_DAILY_PER_CHARACTER = 7,
    ACTION_ON_COMBAT = 23732, // custom value.
}

impl Default for PoiBehavior {
    fn default() -> Self {
        PoiBehavior::DEFAULT
    }
}

fn deserialize_option_string_to_number<'de, D, N>(deserializer: D) -> Result<Option<N>, D::Error>
where
    D: Deserializer<'de>,
    N: FromStr,
{
    let buf = String::deserialize(deserializer)?;
    let val = buf.parse::<N>();
    match val {
        Ok(v) => Ok(Some(v)),
        Err(_) => Ok(None),
    }
}

fn deserialize_string_to_number<'de, D, N>(deserializer: D) -> Result<N, D::Error>
where
    D: Deserializer<'de>,
    N: FromStr,
    <N as FromStr>::Err: Display,
{
    let buf = String::deserialize(deserializer)?;
    let val = buf.parse::<N>();
    match val {
        Ok(v) => Ok(v),
        Err(e) => Err(e).map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Position {
    #[serde(default, deserialize_with = "deserialize_string_to_number")]
    pub xpos: f32,
    #[serde(default, deserialize_with = "deserialize_string_to_number")]
    pub ypos: f32,
    #[serde(default, deserialize_with = "deserialize_string_to_number")]
    pub zpos: f32,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct InheritablePOIData {
    #[serde(
        default,
        rename = "MapID",
        deserialize_with = "deserialize_option_string_to_number"
    )]
    pub map_id: Option<u32>,
    #[serde(rename = "iconFile")]
    pub icon_file: Option<PathBuf>,
    pub guid: Option<String>,
    #[serde(
        default,
        rename = "iconSize",
        deserialize_with = "deserialize_option_string_to_number"
    )]
    pub icon_size: Option<f32>, // = 1.0f;
    #[serde(
        default,
        rename = "alpha",
        deserialize_with = "deserialize_option_string_to_number"
    )]
    pub alpha: Option<f32>, // = 1.0f;
    pub behavior: Option<PoiBehavior>, // = poiBehavior::DEFAULT;
    #[serde(
        default,
        rename = "fadeNear",
        deserialize_with = "deserialize_option_string_to_number"
    )]
    pub fade_near: Option<f32>, // = -1;
    #[serde(
        default,
        rename = "fadeFar",
        deserialize_with = "deserialize_option_string_to_number"
    )]
    pub fade_far: Option<f32>, // = -1;
    #[serde(
        default,
        rename = "heightOffset",
        deserialize_with = "deserialize_option_string_to_number"
    )]
    pub height_offset: Option<f32>,
    pub reset_length: Option<f32>,
    pub display_name: Option<String>,
    pub auto_trigger: Option<bool>,  // = false;
    pub has_countdown: Option<bool>, // = false;
    pub trigger_range: Option<f32>,  // = 5;
    // TODO: max/min size
    pub achievement_id: Option<i32>,
    pub achievement_bit: Option<i32>, // = -1;
    pub info: Option<String>,
    pub info_range: Option<f32>,
    pub is_poi: Option<bool>, // = false;
}

impl InheritablePOIData {
    fn merge(&mut self, other: &InheritablePOIData) {
        self.map_id = self.map_id.or(other.map_id);
        self.icon_file = self.icon_file.clone().or(other.icon_file.clone());
        self.guid = self.guid.clone().or(other.guid.clone());
        self.icon_size = self.icon_size.or(other.icon_size);
        self.alpha = self.alpha.or(other.alpha);
        self.behavior = self.behavior.clone().or(other.behavior.clone());
        self.fade_near = self.fade_near.or(other.fade_near);
        self.fade_far = self.fade_far.or(other.fade_far);
        self.height_offset = self.height_offset.or(other.height_offset);
        self.reset_length = self.reset_length.or(other.reset_length);
        self.display_name = self.display_name.clone().or(other.display_name.clone());
        self.auto_trigger = self.auto_trigger.or(other.auto_trigger);
        self.has_countdown = self.has_countdown.or(other.has_countdown);
        self.trigger_range = self.trigger_range.or(other.trigger_range);

        self.achievement_id = self.achievement_id.or(other.achievement_id);
        self.achievement_bit = self.achievement_bit.or(other.achievement_bit);
        self.info = self.info.clone().or(other.info.clone());
        self.info_range = self.info_range.or(other.info_range);
        self.is_poi = self.is_poi.or(other.is_poi);
    }
}

#[derive(Deserialize, Debug)]
pub enum PoiType {
    Trail(Trail),
    POI(POI),
}

#[derive(Debug, Default, Clone)]
struct TrailData {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
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

                        let trail = TrailData { x, y, z };
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
                    vertices.push(Vertex::new(prev_p1, Vec3::ONE, Vec2::new(0.0, 0.0)));
                    vertices.push(Vertex::new(prev_p2, Vec3::ONE, Vec2::new(1.0, 0.0)));
                    (prev_p1, prev_p2) =
                        Trail::get_perpendicular_point(prev_data, current_data, width);
                    // Calculate distance between the last and current point to adjust the uv
                    // coordinates
                    let distance = prev_data.distance(current_data);
                    // TODO: Fix very long trail segments
                    let frac = 1.0f32.max(distance / width);
                    vertices.push(Vertex::new(prev_p2, Vec3::ONE, Vec2::new(1.0, frac)));
                    vertices.push(Vertex::new(prev_p1, Vec3::ONE, Vec2::new(0.0, frac)));
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
    //cube_mesh.insert_attribute(
    //    Mesh::ATTRIBUTE_COLOR,
    //    vertices.iter().map(|v| v.color).collect::<Vec<Vec3>>(),
    //);
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
    color: Vec3,
    tex_coord: Vec2,
}

impl Vertex {
    pub fn new(pos: Vec3, color: Vec3, tex_coord: Vec2) -> Self {
        Self {
            pos,
            color,
            tex_coord,
        }
    }
}

// TODO: are POI and MarkerCategory effectively the same?
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct POI {
    #[serde(rename = "type")]
    pub poi_type: Option<String>,
    #[serde(flatten)]
    pub pos: Position,
    #[serde(flatten)]
    data: InheritablePOIData,
    #[serde(skip)]
    parent: Option<MarkerCategoryContainer>,
    #[serde(skip)]
    enabled: bool, // = true;
}

impl PoiTrait for POI {
    fn get_parent_category(&self) -> Option<MarkerCategoryContainer> {
        self.parent.clone()
    }
    fn set_parent(&mut self, parent: Option<MarkerCategoryContainer>) {
        self.parent = parent;
    }

    fn get_parent(&self) -> Option<MarkerCategoryContainer> {
        self.parent.clone()
    }
}

macro_rules! getter_setter_poi {
    ($field: expr, $type: ty) => {
        paste::paste! {
            pub fn [<get_ $field>](&self) -> Option<$type>{
                match self.data.clone().$field {
                    Some(data) => Some(data),
                    None => match self.parent.clone() {
                        Some(parent) => parent.read().unwrap().data.[<get_ $field>](),
                        None => None,
                    },
                }
            }

            pub fn [<set_ $field>](&mut self, data: Option<$type>) {
                self.data.$field = data;
            }
        }
    };
}

impl POI {
    // Creates a new POI
    fn new(parent: Option<MarkerCategoryContainer>) -> Self {
        let poi = POI {
            parent: parent.clone(),
            ..Default::default()
        };
        return poi;
    }

    fn set_data(&mut self, data: InheritablePOIData) {
        self.data = data;
    }

    getter_setter_poi!(icon_file, PathBuf);
    getter_setter_poi!(map_id, u32);
    getter_setter_poi!(display_name, String);
    getter_setter_poi!(height_offset, f32);
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use crate::gw2poi::PoiTrait;

    use super::{InheritablePOIData, MarkerCategory, OverlayData, POI};

    #[test]
    fn xml_test() {
        let xml_string = r#"
            <OverlayData>
            <MarkerCategory name="collectible">
            <MarkerCategory name="LionArchKarka" DisplayName="Lion's Arch Exterminator">
            <MarkerCategory name="Part1" DisplayName="Part 1">
            <MarkerCategory name="Karka1" DisplayName="Trail" iconFile="Data\Karkasymbol.png"/>
            <MarkerCategory name="Karkastart1" DisplayName="Start" iconFile="Data\Karkahunt1.png"/>
            <MarkerCategory name="Karkaend1" DisplayName="End" iconFile="Data\KarkasymbolEnd1.png"/>
            </MarkerCategory>
            </MarkerCategory>
            </MarkerCategory>

            <POIs>
            <POI MapID="50" xpos="-300.387" ypos="31.3539" zpos="358.293" type="collectible.LionArchKarka.Part1.Karka1" GUID="BJLO59XWN0u9lzYPrnH16w==" fadeNear="3000" fadeFar="4000"/>
            <Trail type="collectible.LionArchKarka.Part2.Karka2" GUID="gLZdqI4M2EoIO/zrw5KqPg==" trailData="Data/Karkatrail2.trl" texture="Data/Karkahunt.png" color="F78181" alpha="0.8" fadeNear="3000" fadeFar="4000" animSpeed="0"/>
            <POI MapID="50" xpos="-300.387" ypos="31.3539" zpos="358.293" type="collectible.LionArchKarka.Part1.Karka1" GUID="BJLO59XWN0u9lzYPrnH16w==" fadeNear="3000" fadeFar="4000"/>
            <POI MapID="50" xpos="-300.387" ypos="31.3539" zpos="358.293" type="collectible.LionArchKarka.Part1.Karka1" GUID="BJLO59XWN0u9lzYPrnH16w==" fadeNear="3000" fadeFar="4000"/>
            </POIs>
            </OverlayData>
            "#;

        let mut overlay_data: OverlayData = OverlayData::from_string(xml_string);
        overlay_data.fill_poi_parents();

        let parent_opt = overlay_data.pois.poi_list[0].read().unwrap().parent.clone();
        assert!(parent_opt.is_some());
        let parent = parent_opt.unwrap();
        assert_eq!(parent.read().unwrap().name, "Karka1");

        let poi = overlay_data.pois.poi_list[0].read().unwrap();

        assert_eq!(poi.get_map_id().unwrap(), 50);
        assert_eq!(
            poi.poi_type.clone().unwrap(),
            "collectible.LionArchKarka.Part1.Karka1"
        );
        assert_eq!(
            poi.get_icon_file().unwrap().to_str().unwrap(),
            r"Data\Karkasymbol.png"
        );

        assert_eq!(overlay_data.pois.trail_list.len(), 1);
    }

    #[test]
    fn fill_test() {
        let mut overlay_data = OverlayData {
            ..Default::default()
        };
        let category = Arc::new(RwLock::new(MarkerCategory::new()));
        let category2 = Arc::new(RwLock::new(MarkerCategory::new()));
        category2.write().unwrap().data.parent = Some(category.clone());

        category
            .write()
            .unwrap()
            .children
            .insert("category2".into(), category2.clone());
        category2.write().unwrap().name = "category2".into();
        category.write().unwrap().name = "category".into();

        let mut poi = POI::new(Some(category2));
        poi.poi_type = Some("category.category2".into());

        overlay_data.marker_category.push(category);
        overlay_data.pois.poi_list.push(Arc::new(RwLock::new(poi)));

        overlay_data.fill_poi_parents();

        let my_cat = overlay_data.marker_category[0].read().unwrap();
        assert_eq!(overlay_data.marker_category.len(), 1);
        assert_eq!(my_cat.name, "category");
        let child = my_cat.get_category_children("category2").unwrap();
        assert_eq!(child.read().unwrap().name, "category2");
    }

    #[test]
    fn inherit_test() {
        let category = Arc::new(RwLock::new(MarkerCategory::new()));
        category.write().unwrap().data = POI {
            ..Default::default()
        };
        category
            .write()
            .unwrap()
            .data
            .set_icon_file(Some("test_file".into()));
        category
            .write()
            .unwrap()
            .data
            .set_display_name(Some("parent_name".into()));
        let category2 = Arc::new(RwLock::new(MarkerCategory::new()));
        category.write().unwrap().name = "category2".into();
        category2.write().unwrap().name = "category2".into();
        category2.write().unwrap().data.parent = Some(category);

        let mut poi = POI::new(Some(category2));

        assert!(poi.parent.is_some());
        let parent_arc = poi.parent.clone().unwrap();
        let parent = parent_arc.read().unwrap();
        assert_eq!(parent.name, "category2");

        assert_eq!(
            parent.data.get_icon_file().unwrap().to_str().unwrap(),
            "test_file"
        );

        assert_eq!(poi.get_icon_file().unwrap().to_str().unwrap(), "test_file");
        assert_eq!(poi.get_display_name().unwrap(), "parent_name");

        poi.set_display_name(Some("child_name".into()));
        assert_eq!(poi.get_display_name().unwrap(), "child_name");
    }
}
