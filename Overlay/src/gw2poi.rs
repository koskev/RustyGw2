use std::{
    collections::HashMap,
    fmt::Display,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Deserializer, Serialize};

type MarkerCategoryContainer = Arc<RwLock<MarkerCategory>>;
type PoiContainer = Arc<RwLock<POI>>;

fn deserialize_option_path<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    let p = Option::<String>::deserialize(deserializer)?;
    println!("Before: {:?}", p);
    let p = match p {
        Some(p) => Some(PathBuf::from(p.replace(r"\", "/"))),
        None => None,
    };
    println!("After: {:?}", p);

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
    let poi = Vec::<POI>::deserialize(deserializer)?;
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
        println!("{:?}", next_name);
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
    pub fn fill_poi_parents(&mut self) {
        self.pois.poi_list.iter_mut().for_each(|poi| {
            self.marker_category.iter().for_each(|category| {
                let category_name = poi.read().unwrap().poi_type.clone();
                println!("Name: {:?}", category_name);
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
    println!("Converting from {}", buf);
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
    pub map_id: Option<i32>,
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

pub enum PoiType {
    Trail(Trail),
    POI(POI),
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
struct Trail {
    #[serde(rename = "trailData")]
    trail_data: PathBuf,
    texture: PathBuf,
    color: Option<String>,
    #[serde(rename = "animSpeed")]
    anim_speed: f32,
    #[serde(flatten)]
    poi: POI,
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
    getter_setter_poi!(map_id, i32);
    getter_setter_poi!(display_name, String);
}

//std::shared_ptr<POI> get_child(const std::string& name);
//static std::shared_ptr<POI> find_children(const poi_container children, const std::string& name);

//// setter
//void set_name(const std::string& name);
//void set_map_id(int id);
//void set_x(float val);
//void set_y(float val);
//void set_z(float val);
//void set_type(const std::string& type);
//void set_guid(const std::string& uid);
//void set_icon_size(float val);
//void set_icon_file(const std::string& file);
//void set_alpha(float alpha);
//void set_behavior(int i);
//void set_fade_near(float fade);
//void set_fade_far(float fade);
//void set_height_offset(float off);
//void set_reset_length(int len);
//void set_display_name(const std::string& name);
//void set_auto_trigger(bool mode);
//void set_trigger_range(float range);
//void set_has_countdown(bool cd);
//void set_achievement_id(int id);
//void set_achievement_bit(int bit);
//void set_info(const std::string& info);
//void set_info_range(float range);
//void set_is_poi(bool poi);
//void set_pos(const glm::vec3& pos);
//void set_enabled(bool state, bool recursive = true);
//
//// getter
//float get_icon_size() const;
//std::string get_icon_file() const;
//float get_alpha() const;
//int get_behavior() const;
//float get_fade_near() const;
//float get_fade_far() const;
//float get_height_offset() const;
//int get_reset_length() const;
//std::string get_display_name() const;
//bool get_auto_trigger() const;
//float get_trigger_range() const;
//bool get_has_countdown() const;
//int get_achievement_id() const;
//int get_achievement_bit() const;
//std::string get_info() const;
//float get_info_range() const;
//bool get_is_poi() const;
//int get_map_id() const;
//glm::vec3 get_pos() const;
//std::string get_guid() const;
//std::string get_name() const;
//std::string get_type() const;
//bool is_enabled() const;
//
//protected:
//};
//
//typedef POI::poi_container poi_container;
//
//#include "POI.h"
//
//std::shared_ptr<POI> POI::get_child(const std::string& name_case) {
//    std::string name = name_case;
//    std::transform(name.begin(), name.end(), name.begin(), [](unsigned char c) { return std::tolower(c); });
//    std::vector<std::string> tokens;
//    std::stringstream ss(name);
//    std::string s;
//
//    while (std::getline(ss, s, '.')) {
//        tokens.push_back(s);
//    }
//    // not a valid child token
//    if (tokens.size() <= 1) {
//        return nullptr;
//    }
//
//    // striping own name
//    std::string next_name = tokens[1];
//    std::string next_string = name.substr(name.find_first_of(".") + 1);
//    for (auto iter = this->m_children.begin(); iter != this->m_children.end(); ++iter) {
//        // full match!
//        if (next_string == (*iter)->m_name) {
//            return *iter;
//        }
//        // partial match. search children
//        else if (next_name == (*iter)->m_name) {
//            return (*iter)->get_child(next_string);
//        }
//    }
//    return nullptr;
//}
//
//std::shared_ptr<POI> POI::find_children(const poi_container children, const std::string& name) {
//    for (auto iter = children.begin(); iter != children.end(); ++iter) {
//        auto child = (*iter)->get_child(name);
//        if (child) return child;
//    }
//    return nullptr;
//}
//const poi_container* POI::get_children() const { return &this->m_children; }
//void POI::clear_children() { this->m_children.clear(); }
//
//// setter
//void POI::set_name(const std::string& name) { this->m_name = name; }
//void POI::set_map_id(int id) { this->m_inheritable_data.m_map_id = id; }
//void POI::set_x(float val) { this->m_inheritable_data.m_pos.x = val; }
//void POI::set_y(float val) { this->m_inheritable_data.m_pos.y = val; }
//void POI::set_z(float val) { this->m_inheritable_data.m_pos.z = val; }
//void POI::set_type(const std::string& type) { this->m_inheritable_data.m_type = type; }
//void POI::set_guid(const std::string& uid) { this->m_inheritable_data.m_guid = uid; }
//void POI::set_icon_size(float val) { this->m_inheritable_data.m_icon_size = val; }
//void POI::set_icon_file(const std::string& file) {
//    std::string fixed_path = file;
//    std::replace(fixed_path.begin(), fixed_path.end(), '\\', '/');
//    this->m_inheritable_data.m_icon_file = fixed_path;
//}
//
//void POI::set_alpha(float alpha) { this->m_inheritable_data.m_alpha = alpha; }
//void POI::set_behavior(int i) { this->m_inheritable_data.m_behavior = i; }
//void POI::set_fade_near(float fade) { this->m_inheritable_data.m_fade_near = fade; }
//void POI::set_fade_far(float fade) { this->m_inheritable_data.m_fade_far = fade; }
//void POI::set_height_offset(float off) { this->m_inheritable_data.m_height_offset = off; }
//void POI::set_reset_length(int len) { this->m_inheritable_data.m_reset_length = len; }
//void POI::set_display_name(const std::string& name) { this->m_inheritable_data.m_display_name = name; }
//void POI::set_auto_trigger(bool mode) { this->m_inheritable_data.m_auto_trigger = mode; }
//void POI::set_trigger_range(float range) { this->m_inheritable_data.m_trigger_range = range; }
//void POI::set_has_countdown(bool cd) { this->m_inheritable_data.m_has_countdown = cd; }
//void POI::set_achievement_id(int id) { this->m_inheritable_data.m_achievement_id = id; }
//void POI::set_achievement_bit(int bit) { this->m_inheritable_data.m_achievement_bit = bit; }
//void POI::set_info(const std::string& info) { this->m_inheritable_data.m_info = info; }
//void POI::set_info_range(float range) { this->m_inheritable_data.m_info_range = range; }
//void POI::set_is_poi(bool poi) { this->m_inheritable_data.m_is_poi = poi; }
//void POI::set_pos(const glm::vec3& pos) { this->m_inheritable_data.m_pos = pos; }
//void POI::set_enabled(bool state, bool recursive) {
//    if (this->m_enabled == state) return;
//    this->m_enabled = state;
//    if (recursive) {
//        for (auto child = this->m_children.begin(); child != this->m_children.end(); ++child) {
//            child->get()->set_enabled(state, true);
//        }
//    }
//    if (state) {
//        auto parent = this->get_parent().read();
//        if (parent) {
//            parent->set_enabled(state, false);
//        }
//    }
//}
//
//// getter
//float POI::get_icon_size() const { return this->m_inheritable_data.m_icon_size; }
//std::string POI::get_icon_file() const { return this->m_inheritable_data.m_icon_file; }
//float POI::get_alpha() const { return this->m_inheritable_data.m_alpha; }
//int POI::get_behavior() const { return this->m_inheritable_data.m_behavior; }
//float POI::get_fade_near() const { return this->m_inheritable_data.m_fade_near; }
//float POI::get_fade_far() const { return this->m_inheritable_data.m_fade_far; }
//float POI::get_height_offset() const { return this->m_inheritable_data.m_height_offset; }
//int POI::get_reset_length() const { return this->m_inheritable_data.m_reset_length; }
//std::string POI::get_display_name() const { return this->m_inheritable_data.m_display_name; }
//bool POI::get_auto_trigger() const { return this->m_inheritable_data.m_auto_trigger; }
//float POI::get_trigger_range() const { return this->m_inheritable_data.m_trigger_range; }
//bool POI::get_has_countdown() const { return this->m_inheritable_data.m_has_countdown; }
//int POI::get_achievement_id() const { return this->m_inheritable_data.m_achievement_id; }
//int POI::get_achievement_bit() const { return this->m_inheritable_data.m_achievement_bit; }
//std::string POI::get_info() const { return this->m_inheritable_data.m_info; }
//float POI::get_info_range() const { return this->m_inheritable_data.m_info_range; }
//bool POI::get_is_poi() const { return this->m_inheritable_data.m_is_poi; }
//int POI::get_map_id() const { return this->m_inheritable_data.m_map_id; }
//glm::vec3 POI::get_pos() const { return this->m_inheritable_data.m_pos; }
//std::string POI::get_guid() const { return this->m_inheritable_data.m_guid; }
//std::string POI::get_name() const { return this->m_name; }
//std::string POI::get_type() const { return this->m_inheritable_data.m_type; }
//bool POI::is_enabled() const { return this->m_enabled; }
//
//

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
            </POIs>
            </OverlayData>
            "#;

        let mut overlay_data: OverlayData = serde_xml_rs::from_str(&xml_string).unwrap();
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
