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
