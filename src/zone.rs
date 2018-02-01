extern crate geo;
extern crate geojson;
extern crate itertools;
extern crate mimir;
extern crate mimirsbrunn;
extern crate serde;

use std::rc::Rc;
use self::itertools::Itertools;
use self::mimir::{Coord, Property};
use osmpbfreader::objects::{OsmId, OsmObj, Relation};
use self::mimirsbrunn::boundaries::{build_boundary, make_centroid};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Copy, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ZoneType {
    Suburb,
    CityDistrict,
    City,
    StateDistrict,
    State,
    CountryRegion,
    Country,
    NonAdministrative,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Zone {
    pub id: String,
    pub admin_level: Option<u32>,
    pub zone_type: Option<ZoneType>,
    pub name: String,
    pub zip_codes: Vec<String>,
    pub center: Option<Coord>,
    #[serde(serialize_with = "serialize_as_geojson", deserialize_with = "deserialize_as_geojson",
            rename = "geometry", default)]
    pub boundary: Option<geo::MultiPolygon<f64>>,
    pub parent: Option<Rc<Zone>>,
    pub tags: Vec<Property>,
    pub wikidata: Option<String>,
    // pub links: Vec<Rc<Zone>>
}

impl Zone {
    pub fn is_admin(&self) -> bool {
        match self.zone_type {
            None => false,
            Some(ZoneType::NonAdministrative) => false,
            _ => true,
        }
    }

    pub fn from_osm(relation: &Relation) -> Option<Self> {
        // Skip administrative region without name
        let name = match relation.tags.get("name") {
            Some(val) => val,
            None => {
                warn!(
                    "relation/{}: adminstrative region without name, skipped",
                    relation.id.0
                );
                return None;
            }
        };
        let level = relation
            .tags
            .get("admin_level")
            .and_then(|s| s.parse().ok());

        let zip_code = relation
            .tags
            .get("addr:postcode")
            .or_else(|| relation.tags.get("postal_code"))
            .map_or("", |val| &val[..]);
        let zip_codes = zip_code
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .sorted();
        let wikidata = relation.tags.get("wikidata").map(|s| s.to_string());

        Some(Self {
            id: relation.id.0.to_string(),
            admin_level: level,
            zone_type: None,
            name: name.to_string(),
            zip_codes: zip_codes,
            center: None,
            boundary: None,
            parent: None,
            tags: vec![],
            wikidata: wikidata,
        })
    }

    pub fn from_osm_with_geom(
        relation: &Relation,
        objects: &BTreeMap<OsmId, OsmObj>,
    ) -> Option<Self> {
        Self::from_osm(relation).map(|mut result| {
            result.boundary = build_boundary(relation, objects);

            result.center = Some(
                relation
                    .refs
                    .iter()
                    .find(|r| r.role == "admin_centre")
                    .and_then(|r| objects.get(&r.member))
                    .and_then(|o| o.node())
                    .map_or(make_centroid(&result.boundary), |node| {
                        mimir::Coord::new(node.lat(), node.lon())
                    }),
            );

            result
        })
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
    use self::geojson::{GeoJson, Geometry, Value};
    use self::serde::Serialize;

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
    use self::geojson;
    use self::serde::Deserialize;
    use self::geojson::conversion::TryInto;

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
