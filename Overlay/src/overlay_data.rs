use std::{error::Error, fs};

use bevy::prelude::info;
use serde::Deserialize;

use crate::{
    gw2poi::{
        deserialize_marker_category_vec, deserialize_poi_vec, MarkerCategoryContainer,
        PoiContainer, PoiTrait,
    },
    trail::{deserialize_trail_vec, TrailContainer},
};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OverlayData {
    #[serde(
        rename = "MarkerCategory",
        deserialize_with = "deserialize_marker_category_vec",
        default
    )]
    pub marker_category: Vec<MarkerCategoryContainer>,
    #[serde(rename = "POIs", default)]
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
    #[serde(rename = "POI", deserialize_with = "deserialize_poi_vec", default)]
    pub poi_list: Vec<PoiContainer>,
    #[serde(rename = "Trail", deserialize_with = "deserialize_trail_vec", default)]
    pub trail_list: Vec<TrailContainer>,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use walkdir::WalkDir;

    use crate::{
        gw2poi::{MarkerCategory, PoiTrait, POI},
        overlay_data::OverlayData,
    };

    #[test]
    fn xml_file_test() {
        let mut overlay_data: OverlayData = OverlayData::default();
        for entry in WalkDir::new("../pois").into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() && entry.path().extension().unwrap_or_default() == "xml"
            {
                println!("Found XML file: {:?}", entry.path());
                let file_path = entry.path().to_string_lossy().to_string();
                let data = OverlayData::from_file(&file_path);
                overlay_data.merge(data.unwrap());
            }
        }

        overlay_data.fill_poi_parents();
    }

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
            <POI MapID="50" xpos="-300.387" ypos="31.3539" zpos="358.293" type="collectible.LionArchKarka.Part1.Karka1" GUID="BJLO59XWN0u9lzYPrnH16w==" fadeNear="3000" fadeFar="4000" behavior="3" triggerRange="5"/>
            </POIs>
            </OverlayData>
            "#;

        let mut overlay_data: OverlayData = OverlayData::from_string(xml_string);
        overlay_data.fill_poi_parents();

        let parent_opt = overlay_data.pois.poi_list[0].read().unwrap().get_parent();
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
        category2
            .write()
            .unwrap()
            .data
            .set_parent(Some(category.clone()));

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
        category.write().unwrap().data = POI::new(None);
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
        category2.write().unwrap().data.set_parent(Some(category));

        let mut poi = POI::new(Some(category2));

        assert!(poi.get_parent().is_some());
        let parent_arc = poi.get_parent().unwrap();
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
