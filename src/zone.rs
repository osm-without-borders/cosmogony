extern crate geo;
extern crate serde;

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
    #[serde(serialize_with = "serialize_as_geojson", deserialize_with = "deserialize_as_geojson",
            rename = "geometry", default)]
    pub boundary: Option<geo::MultiPolygon<f64>>,
    pub parent: Option<Rc<Zone>>,
    pub tags: Vec<Property>,
    // pub links: Vec<Rc<Zone>>
}

impl Zone {
    pub fn is_admin(&self) -> bool {
        match self.admin_type {
            None => false,
            Some(AdminType::NonAdministrative) => false,
            _ => true,
        }
    }
}

// those 2 methods have been shamelessly copied from https://github.com/CanalTP/mimirsbrunn/blob/master/libs/mimir/src/objects.rs#L277
// see if there is a good way to share the code
fn serialize_as_geojson<S>(
    multi_polygon_option: &Option<geo::MultiPolygon<f64>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use geojson::{GeoJson, Geometry, Value};
    use serde::Serialize;

    match *multi_polygon_option {
        Some(ref multi_polygon) => {
            GeoJson::Geometry(Geometry::new(Value::from(multi_polygon))).serialize(serializer)
        }
        None => serializer.serialize_none(),
    }
}

fn deserialize_as_geojson<'de, D>(d: D) -> Result<Option<geo::MultiPolygon<f64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use geojson;
    use serde::Deserialize;
    use geojson::conversion::TryInto;

    Option::<geojson::GeoJson>::deserialize(d).map(|option| {
        option.and_then(|geojson| match geojson {
            geojson::GeoJson::Geometry(geojson_geom) => {
                let geo_geom: Result<geo::Geometry<f64>, _> = geojson_geom.value.try_into();
                match geo_geom {
                    Ok(geo::Geometry::MultiPolygon(geo_multi_polygon)) => Some(geo_multi_polygon),
                    Ok(_) => None,
                    Err(e) => {
                        warn!("Error deserializing geometry: {}", e);
                        None
                    }
                }
            }
            _ => None,
        })
    })
}
