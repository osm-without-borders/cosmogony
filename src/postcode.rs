use cosmogony::mutable_slice::MutableSlice;
use geo_types::{Coordinate, Geometry, MultiPolygon, Point, Rect, Polygon};
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
    pub boundary: geo_types::MultiPolygon<f64>,
    pub area: f64
}

impl Postcode {
    pub fn get_boundary(&self) -> &geo_types::MultiPolygon<f64> {
        return &self.boundary
    }

    pub fn unsigned_area(&self) -> f64 {
        return self.area;
    }
}

impl Default for Postcode {
    fn default() -> Self {
        Postcode {
            osm_id: "".into(),
            boundary: MultiPolygon(vec![]),
            zipcode: "".into(),
            area: 0.0
        }
    }
}

impl Postcode {}
