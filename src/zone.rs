extern crate geo;
extern crate geojson;
extern crate geos;
extern crate itertools;
extern crate serde;

use self::itertools::Itertools;
use osmpbfreader::objects::{OsmId, OsmObj, Relation, Tags};
use osm_boundaries_utils::build_boundary;
use std::collections::BTreeMap;
use self::geos::GGeom;
use self::serde::Serialize;
use std::fmt;
use geo::Point;

type Coord = Point<f64>;

#[derive(Serialize, Deserialize, Copy, Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
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

#[derive(Debug, Clone)]
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
    pub zip_codes: Vec<String>,
    #[serde(serialize_with = "serialize_as_geojson", deserialize_with = "deserialize_as_coord")]
    pub center: Option<Coord>,
    #[serde(serialize_with = "serialize_as_geojson",
            deserialize_with = "deserialize_as_multipolygon", rename = "geometry", default)]
    pub boundary: Option<geo::MultiPolygon<f64>>,

    pub tags: Tags,

    pub parent: Option<ZoneIndex>,
    pub wikidata: Option<String>,
    // pub links: Vec<ZoneIndex>
}

impl Zone {
    pub fn is_admin(&self) -> bool {
        match self.zone_type {
            None => false,
            Some(ZoneType::NonAdministrative) => false,
            _ => true,
        }
    }

    pub fn set_parent(&mut self, idx: Option<ZoneIndex>) {
        self.parent = idx;
    }

    pub fn from_osm(relation: &Relation, index: ZoneIndex) -> Option<Self> {
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
            id: index,
            osm_id: relation.id.0.to_string(),
            admin_level: level,
            zone_type: None,
            name: name.to_string(),
            zip_codes: zip_codes,
            center: None,
            boundary: None,
            parent: None,
            tags: relation.tags.clone(),
            wikidata: wikidata,
        })
    }

    pub fn from_osm_with_geom(
        relation: &Relation,
        objects: &BTreeMap<OsmId, OsmObj>,
        index: ZoneIndex,
    ) -> Option<Self> {
        use geo::centroid::Centroid;
        Self::from_osm(relation, index).map(|mut result| {
            result.boundary = build_boundary(relation, objects);

            result.center = relation
                .refs
                .iter()
                .find(|r| r.role == "admin_centre")
                .and_then(|r| objects.get(&r.member))
                .and_then(|o| o.node())
                .map_or(
                    result.boundary.as_ref().and_then(|b| b.centroid()),
                    |node| Some(Coord::new(node.lon(), node.lat())),
                );

            result
        })
    }

    pub fn contains(&self, other: &Zone) -> bool {
        return match (&self.boundary, &other.boundary) {
            (&Some(ref mpoly1), &Some(ref mpoly2)) => {
                let m_self: GGeom = mpoly1.into();
                let m_other: GGeom = mpoly2.into();

                // In GEOS, "covers" is less strict than "contains".
                // eg: a polygon does NOT "contain" its boundary, but "covers" it.
                m_self.covers(&m_other)
            }
            _ => false,
        };
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
    use self::geojson::{GeoJson, Geometry, Value};
    use self::serde::Serialize;

    match *multi_polygon_option {
        Some(ref multi_polygon) => {
            GeoJson::Geometry(Geometry::new(Value::from(multi_polygon))).serialize(serializer)
        }
        None => serializer.serialize_none(),
    }
}

fn deserialize_geom<'de, D>(d: D) -> Result<Option<geo::Geometry<f64>>, D::Error>
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

fn deserialize_as_multipolygon<'de, D>(d: D) -> Result<Option<geo::MultiPolygon<f64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match deserialize_geom(d)? {
        Some(geo::Geometry::MultiPolygon(geo_multi_polygon)) => Ok(Some(geo_multi_polygon)),
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
        Some(geo::Geometry::Point(p)) => Ok(Some(p)),
        None => Ok(None),
        Some(_) => Err(serde::de::Error::custom(
            "invalid geometry type, should be a point",
        )),
    }
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

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a zone index")
    }
}
