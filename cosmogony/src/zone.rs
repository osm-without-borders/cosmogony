use crate::mutable_slice::MutableSlice;
use geo_types::{Coordinate, Geometry, MultiPolygon, Point, Rect};
use log::warn;
use osmpbfreader::objects::Tags;
use serde::Serialize;
use serde_derive::*;
use std::collections::BTreeMap;
use std::fmt;

pub type Coord = Point<f64>;

#[derive(Serialize, Deserialize, Copy, Debug, Clone, Eq, Hash, PartialEq, Ord, PartialOrd)]
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

impl ZoneType {
    pub fn as_str(&self) -> &'static str {
        match *self {
            ZoneType::Suburb => "suburb",
            ZoneType::CityDistrict => "city_district",
            ZoneType::City => "city",
            ZoneType::StateDistrict => "state_district",
            ZoneType::State => "state",
            ZoneType::CountryRegion => "country_region",
            ZoneType::Country => "country",
            ZoneType::NonAdministrative => "non_administrative",
        }
    }
}

#[derive(Copy, Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ZoneIndex {
    pub index: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Zone {
    pub id: ZoneIndex,
    pub osm_id: String,
    pub admin_level: Option<u32>,
    pub zone_type: Option<ZoneType>,
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub international_labels: BTreeMap<String, String>,
    // we do not serialize the internal_names,
    // it's only used temporary to build the international_labels
    #[serde(skip)]
    pub international_names: BTreeMap<String, String>, // TODO can we store it outside the zone ?
    pub zip_codes: Vec<String>,
    #[serde(
        serialize_with = "serialize_as_geojson",
        deserialize_with = "deserialize_as_coord"
    )]
    pub center: Option<Coord>,
    #[serde(
        serialize_with = "serialize_as_geojson",
        deserialize_with = "deserialize_as_multipolygon",
        rename = "geometry",
        default
    )]
    pub boundary: Option<geo_types::MultiPolygon<f64>>,

    #[serde(
        serialize_with = "serialize_bbox_as_geojson",
        deserialize_with = "deserialize_as_rect",
        default
    )]
    pub bbox: Option<Rect<f64>>,

    pub tags: Tags,
    #[serde(default = "Tags::new")] //to keep the retrocompatibility with cosmogony2mimir
    pub center_tags: Tags,

    pub parent: Option<ZoneIndex>,
    pub wikidata: Option<String>,
    // pub links: Vec<ZoneIndex>
    #[serde(default)]
    pub is_generated: bool,
    pub country_code: Option<String>,
}

impl Default for Zone {
    fn default() -> Self {
        Zone {
            id: ZoneIndex { index: 0 },
            osm_id: "".into(),
            admin_level: None,
            zone_type: None,
            name: "".into(),
            label: "".into(),
            international_labels: BTreeMap::default(),
            international_names: BTreeMap::default(),
            center: None,
            boundary: None,
            bbox: None,
            parent: None,
            tags: Tags::new(),
            center_tags: Tags::new(),
            wikidata: None,
            zip_codes: vec![],
            is_generated: true,
            country_code: None,
        }
    }
}

impl Zone {
    pub fn is_admin(&self) -> bool {
        matches!(self.zone_type, Some(t) if t!= ZoneType::NonAdministrative)
    }

    pub fn admin_type(&self) -> Option<ZoneType> {
        match self.zone_type {
            Some(t) if t != ZoneType::NonAdministrative => Some(t),
            _ => None
        }
    }

    pub fn set_parent(&mut self, idx: Option<ZoneIndex>) {
        self.parent = idx;
    }

    /// iter_hierarchy gives an iterator over the whole hierachy including self
    pub fn iter_hierarchy<'a>(&'a self, all_zones: &'a MutableSlice<'_>) -> HierarchyIterator<'a> {
        HierarchyIterator {
            zone: Some(&self),
            all_zones,
        }
    }
}

pub struct HierarchyIterator<'a> {
    zone: Option<&'a Zone>,
    all_zones: &'a MutableSlice<'a>,
}

impl<'a> Iterator for HierarchyIterator<'a> {
    type Item = &'a Zone;
    fn next(&mut self) -> Option<&'a Zone> {
        let z = self.zone;
        match z {
            Some(z) => {
                self.zone = match &z.parent {
                    Some(ref p_idx) => Some(self.all_zones.get(&p_idx)),
                    _ => None,
                };
                Some(z)
            }
            None => None,
        }
    }
}

// those 2 methods have been shamelessly copied from https://github.com/CanalTP/mimirsbrunn/blob/master/libs/mimir/src/objects.rs#L277
// see if there is a good way to share the code
fn serialize_as_geojson<'a, S, T>(
    multi_polygon_option: &'a Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    geojson::Value: From<&'a T>,
    S: serde::Serializer,
{
    use geojson::{GeoJson, Geometry, Value};

    match *multi_polygon_option {
        Some(ref multi_polygon) => {
            GeoJson::Geometry(Geometry::new(Value::from(multi_polygon))).serialize(serializer)
        }
        None => serializer.serialize_none(),
    }
}

fn deserialize_geom<'de, D>(d: D) -> Result<Option<Geometry<f64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    use std::convert::TryInto;

    Option::<geojson::GeoJson>::deserialize(d).map(|option| {
        option.and_then(|geojson| match geojson {
            geojson::GeoJson::Geometry(geojson_geom) => {
                let geo_geom: Result<Geometry<f64>, _> = geojson_geom.value.try_into();
                match geo_geom {
                    Ok(g) => Some(g),
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

fn deserialize_as_multipolygon<'de, D>(d: D) -> Result<Option<MultiPolygon<f64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match deserialize_geom(d)? {
        Some(Geometry::MultiPolygon(geo_multi_polygon)) => Ok(Some(geo_multi_polygon)),
        None => Ok(None),
        Some(_) => Err(serde::de::Error::custom(
            "invalid geometry type, should be a multipolygon",
        )),
    }
}

fn deserialize_as_coord<'de, D>(d: D) -> Result<Option<Coord>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match deserialize_geom(d)? {
        Some(Geometry::Point(p)) => Ok(Some(p)),
        None => Ok(None),
        Some(_) => Err(serde::de::Error::custom(
            "invalid geometry type, should be a point",
        )),
    }
}

fn serialize_bbox_as_geojson<S>(bbox: &Option<Rect<f64>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use geojson::Bbox as GeojsonBbox;
    match bbox {
        Some(b) => {
            // bbox serialized as an array
            // using GeoJSON bounding box format
            // See RFC 7946: https://tools.ietf.org/html/rfc7946#section-5
            let geojson_bbox: GeojsonBbox = vec![b.min().x, b.min().y, b.max().x, b.max().y];
            geojson_bbox.serialize(serializer)
        }
        None => serializer.serialize_none(),
    }
}

fn deserialize_as_rect<'de, D>(d: D) -> Result<Option<Rect<f64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    Option::<Vec<f64>>::deserialize(d).map(|option| match option {
        Some(b) => Some(Rect::new(
            Coordinate { x: b[0], y: b[1] }, // min
            Coordinate { x: b[2], y: b[3] }, // max
        )),
        None => None,
    })
}

impl Serialize for ZoneIndex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.index as u64)
    }
}

impl<'de> serde::Deserialize<'de> for ZoneIndex {
    fn deserialize<D>(deserializer: D) -> Result<ZoneIndex, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_u64(ZoneIndexVisitor)
    }
}

struct ZoneIndexVisitor;

impl<'de> serde::de::Visitor<'de> for ZoneIndexVisitor {
    type Value = ZoneIndex;

    fn visit_u64<E>(self, data: u64) -> Result<ZoneIndex, E>
    where
        E: serde::de::Error,
    {
        Ok(ZoneIndex {
            index: data as usize,
        })
    }

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a zone index")
    }
}
