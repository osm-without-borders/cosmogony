extern crate geo;

use std::rc::Rc;
use mimir::{Coord, Property};

use admin_type::AdminType;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Zone {
    pub id: String,
    pub admin_level: Option<u32>,
    pub admin_type: Option<AdminType>,
    pub name: String,
    pub zip_codes: Vec<String>,
    pub center: Coord,
    pub boundary: Option<geo::MultiPolygon<f64>>,
    pub parent: Option<Rc<Zone>>,
    pub tags: Vec<Property>,
    // pub links: Vec<Rc<Zone>>
}

impl Zone {
    fn is_admin(&self) -> bool {
        match self.admin_type {
            None => false,
            Some(AdminType::NonAdministrative) => false,
            _ => true,
        }
    }
}
