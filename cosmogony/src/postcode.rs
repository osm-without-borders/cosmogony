use crate::mutable_slice::MutableSlice;
use geo_types::{Coordinate, Geometry, MultiPolygon, Point, Rect};
use log::warn;
use osmpbfreader::objects::Tags;
use serde::Serialize;
use serde_derive::*;
use std::collections::BTreeMap;
use std::fmt;

pub type Coord = Point<f64>;

#[derive(Debug, Clone)]
pub struct Postcode {
    pub osm_id: String,
    pub zipcode: String,
    pub boundary: Option<geo_types::MultiPolygon<f64>>,
}

impl Default for Postcode {
    fn default() -> Self {
        Postcode {
            osm_id: "".into(),
            boundary: None,
            zipcode: "".into(),
        }
    }
}

impl Postcode {
}
