use std::{
    collections::HashMap,
    fmt::Display,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Deserializer};

pub type MarkerCategoryContainer = Arc<RwLock<MarkerCategory>>;
pub type PoiContainer = Arc<RwLock<POI>>;

pub fn deserialize_option_path<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
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

pub fn deserialize_marker_category_hashmap<'de, D>(
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

pub fn deserialize_marker_category_vec<'de, D>(
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

pub fn deserialize_poi_vec<'de, D>(deserializer: D) -> Result<Vec<PoiContainer>, D::Error>
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

#[derive(Debug, Clone, Deserialize)]
pub enum PoiBehavior {
    Default = 0,
    ReappearOnMapChange = 1,
    ReappearOnDailyReset = 2,
    OnlyVisibleBeforeActivation = 3,
    ReappearAfterTimer = 4,
    ReappearOnMapReset = 5,
    OncePerInstance = 6,
    OnceDailyPerCharacter = 7,
    ActionOnCombat = 23732, // custom value.
}

impl Default for PoiBehavior {
    fn default() -> Self {
        PoiBehavior::Default
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

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Position {
    #[serde(default, deserialize_with = "deserialize_string_to_number")]
    pub xpos: f32,
    #[serde(default, deserialize_with = "deserialize_string_to_number")]
    pub ypos: f32,
    #[serde(default, deserialize_with = "deserialize_string_to_number")]
    pub zpos: f32,
}

#[allow(dead_code)]
#[derive(Debug, Default, Clone, Deserialize)]
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
    pub color: Option<String>,
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

// TODO: are POI and MarkerCategory effectively the same?
#[allow(dead_code)]
#[derive(Debug, Default, Deserialize, Clone)]
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

#[allow(dead_code)]
impl POI {
    // Creates a new POI
    pub fn new(parent: Option<MarkerCategoryContainer>) -> Self {
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
    getter_setter_poi!(fade_near, f32);
    getter_setter_poi!(fade_far, f32);
    getter_setter_poi!(alpha, f32);
    getter_setter_poi!(icon_size, f32);
}
