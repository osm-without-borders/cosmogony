use crate::mutable_slice::MutableSlice;
use geo::algorithm::bounding_rect::BoundingRect;
use geo::prelude::Contains;
use geo_types::{Coordinate, Point, Rect};
use geos::GGeom;
use itertools::Itertools;
use log::{debug, info, warn};
use osm_boundaries_utils::build_boundary;
use osmpbfreader::objects::{OsmId, OsmObj, Relation, Tags, Node};
use regex::Regex;
use serde::Serialize;
use serde_derive::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

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

#[derive(Copy, Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
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
    international_names: BTreeMap<String, String>,
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
}

/// get all the international names from the osm tags
///
/// the names in osm are in a tag names `name:<lang>`,
/// eg `name:fr`, `name:de`, ...
///
/// we don't add the international names that are equivalent to the default name
/// to reduce the size of the map
fn get_international_names(tags: &Tags, default_name: &str) -> BTreeMap<String, String> {
    lazy_static::lazy_static! {
        static ref LANG_NAME_REG: Regex = Regex::new("^name:(.+)").unwrap();
    }

    tags.iter()
        .filter(|&(_, v)| v != default_name)
        .filter_map(|(k, v)| {
            let lang = LANG_NAME_REG.captures(k)?.get(1)?;

            Some((lang.as_str().into(), v.clone()))
        }).collect()
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
        }
    }
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

    pub fn from_osm_node(
        node: &Node,
        index: ZoneIndex,
    ) -> Option<Self> {
        let osm_id = OsmId::Node(node.id);
        let osm_id_str = match osm_id {
            OsmId::Node(n) => format!("node:{}", n.0.to_string()),
            OsmId::Relation(r) => format!("relation:{}", r.0.to_string()),
            OsmId::Way(r) => format!("way:{}", r.0.to_string()),
        };
        let tags = &node.tags;
        let name = match tags.get("name") {
            Some(val) => val,
            None => {
                debug!(
                    "{}: administrative region without name, skipped",
                    &osm_id_str
                );
                return None;
            }
        };
        let level = tags.get("admin_level").and_then(|s| s.parse().ok());
        let zip_code = tags
            .get("addr:postcode")
            .or_else(|| tags.get("postal_code"))
            .map_or("", |val| &val[..]);
        let zip_codes = zip_code
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .sorted()
            .collect();
        let wikidata = tags.get("wikidata").map(|s| s.to_string());

        let international_names = get_international_names(&tags, name);
        Some(Self {
            id: index,
            osm_id: osm_id_str,
            admin_level: level,
            zone_type: None,
            name: name.to_string(),
            boundary: None,
            bbox: None,
            parent: None,
            tags: tags.clone(),
            center_tags: Tags::new(),
            wikidata,
            center: None,
            international_labels: BTreeMap::default(),
            international_names,
            label: "".to_string(),
            zip_codes,
            is_generated: true,
        })
    }

    pub fn from_osm(
        relation: &Relation,
        objects: &BTreeMap<OsmId, OsmObj>,
        index: ZoneIndex,
    ) -> Option<Self> {
        // Skip administrative region without name
        let name = match relation.tags.get("name") {
            Some(val) => val,
            None => {
                debug!(
                    "relation/{}: administrative region without name, skipped",
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
            .sorted()
            .collect();
        let wikidata = relation.tags.get("wikidata").map(|s| s.to_string());

        let label_node = relation
            .refs
            .iter()
            .find(|r| r.role == "label")
            .and_then(|r| objects.get(&r.member))
            .and_then(|o| o.node());

        let mut tags = relation.tags.clone();
        if let Some(node) = label_node {
            node.tags
                .iter()
                .filter(|(k, _)| k.starts_with("name:"))
                .for_each(|(k, v)| {
                    tags.entry(k.to_string()).or_insert(v.to_string());
                })
        }

        Some(Self {
            id: index,
            osm_id: format!("relation:{}", relation.id.0.to_string()), // for the moment we can only read relation
            admin_level: level,
            zone_type: None,
            name: name.to_string(),
            label: "".to_string(),
            international_labels: BTreeMap::default(),
            international_names: BTreeMap::default(),
            zip_codes,
            center: None,
            boundary: None,
            bbox: None,
            parent: None,
            tags,
            center_tags: Tags::new(),
            wikidata,
            is_generated: false,
        })
    }

    pub fn from_osm_with_geom(
        relation: &Relation,
        objects: &BTreeMap<OsmId, OsmObj>,
        index: ZoneIndex,
    ) -> Option<Self> {
        use geo::centroid::Centroid;
        Self::from_osm(relation, objects, index).map(|mut result| {
            result.boundary = build_boundary(relation, objects);
            result.bbox = result.boundary.as_ref().and_then(|b| b.bounding_rect());
            result.is_generated = false;

            let refs = &relation.refs;
            let center = refs
                .iter()
                .find(|r| r.role == "admin_centre")
                .or(refs.iter().find(|r| r.role == "label"))
                .and_then(|r| objects.get(&r.member))
                .and_then(|o| o.node());

            result.center = center.map_or(
                result.boundary.as_ref().and_then(|b| {
                    b.centroid().filter(|p| {
                        /*
                            On a broken polygon Geo may return Some(NaN,NaN) centroid.
                            It should NOT be serialized as [null,null] in the JSON output.
                        */
                        if p.x().is_nan() || p.y().is_nan() {
                            warn!("NaN in centroid {:?} for {}", p, result.osm_id);
                            return false;
                        }
                        return true;
                    })
                }),
                |node| Some(Coord::new(node.lon(), node.lat())),
            );

            result.center_tags = center.map_or(Tags::new(), |n| n.tags.clone());

            result
        })
    }

    pub fn contains(&self, other: &Zone) -> bool {
        use geos::from_geo::TryInto;
        match (&self.boundary, &other.boundary) {
            (&Some(ref mpoly1), &Some(ref mpoly2)) => {
                let m_self: Result<GGeom, _> = mpoly1.try_into();
                let m_other: Result<GGeom, _> = mpoly2.try_into();

                match (&m_self, &m_other) {
                    (&Ok(ref m_self), &Ok(ref m_other)) => {
                        // In GEOS, "covers" is less strict than "contains".
                        // eg: a polygon does NOT "contain" its boundary, but "covers" it.
                        m_self.covers(&m_other)
                        .map_err(|e| info!("impossible to compute geometies coverage for zone {:?}/{:?}: error {}",
                        &self.osm_id, &other.osm_id, e))
                        .unwrap_or(false)
                    }
                    (&Err(ref e), _) => {
                        info!(
                            "impossible to convert to geos for zone {:?}, error {}",
                            &self.osm_id, e
                        );
                        debug!(
                            "impossible to convert to geos the zone {:?}",
                            serde_json::to_string(&self)
                        );
                        false
                    }
                    (_, &Err(ref e)) => {
                        info!(
                            "impossible to convert to geos for zone {:?}, error {}",
                            &other.osm_id, e
                        );
                        debug!(
                            "impossible to convert to geos the zone {:?}",
                            serde_json::to_string(&other)
                        );
                        false
                    }
                }
            }
            _ => false,
        }
    }

    // TODO factorize it with contains
    pub fn contains_center(&self, other: &Zone) -> bool {
        match (&self.boundary, &other.center) {
            (&Some(ref mpoly1), &Some(ref point)) => mpoly1.contains(point),
            _ => false,
        }
    }

    /// iter_hierarchy gives an iterator over the whole hierachy including self
    pub fn iter_hierarchy<'a>(&'a self, all_zones: &'a MutableSlice<'_>) -> HierarchyIterator<'a> {
        HierarchyIterator {
            zone: Some(&self),
            all_zones: all_zones,
        }
    }

    fn create_lbl<'a, F>(&'a self, all_zones: &'a MutableSlice<'_>, f: F) -> String
    where
        F: Fn(&Zone) -> String,
    {
        let mut hierarchy: Vec<String> = self.iter_hierarchy(all_zones).map(f).dedup().collect();

        if let Some(ref mut zone_name) = hierarchy.first_mut() {
            zone_name.push_str(&format_zip_code(&self.zip_codes));
        }
        hierarchy.join(", ")
    }
    /// compute a nice human readable label
    /// The label carries the hierarchy of a zone.
    ///
    /// This label is inspired from
    /// [opencage formatting](https://blog.opencagedata.com/post/99059889253/good-looking-addresses-solving-the-berlin-berlin)
    ///
    /// and from the [mimirsbrunn](https://github.com/CanalTP/mimirsbrunn) zip code formatting
    ///
    /// example of zone's label:
    /// Paris (75000-75116), Île-de-France, France
    ///
    /// We compute a default label, and a label per language
    /// Note: for the moment we use the same format for every language,
    /// but in the future we might use opencage's configuration for this
    pub fn compute_labels(&mut self, all_zones: &MutableSlice<'_>) {
        let label = self.create_lbl(all_zones, |z: &Zone| z.name.clone());

        // we compute a label per language
        let all_lang: BTreeSet<String> = self
            .iter_hierarchy(all_zones)
            .map(|z| z.international_names.keys())
            .flat_map(|i| i)
            .map(|n| n.as_str().into())
            .collect();

        let international_labels = all_lang
            .iter()
            .map(|lang| {
                let lbl = self.create_lbl(all_zones, |z: &Zone| {
                    z.international_names.get(lang).unwrap_or(&z.name).clone()
                });
                (lang.to_string(), lbl)
            }).collect();

        self.international_labels = international_labels;
        self.label = label;
    }

    pub fn compute_names(&mut self) {
        if self.zone_type == Some(ZoneType::City)
            || self.wikidata.is_some()
                && self.wikidata == self.center_tags.get("wikidata").map(|s| s.to_string())
        {
            let center_names: Vec<_> = self
                .center_tags
                .iter()
                .filter(|(k, _)| k.starts_with("name:"))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();

            center_names.into_iter().for_each(|(k, v)| {
                self.tags.entry(k).or_insert(v);
            })
        }
        self.international_names = get_international_names(&self.tags, &self.name);
    }
}

/// format the zone's zip code
/// if no zipcode, we return an empty string
/// if only one zipcode, we return it between ()
/// if more than one we display the range of zips code
///
/// This way for example Paris will get " (75000-75116)"
///
/// ruthlessly taken from mimir
fn format_zip_code(zip_codes: &[String]) -> String {
    match zip_codes.len() {
        0 => "".to_string(),
        1 => format!(" ({})", zip_codes.first().unwrap()),
        _ => format!(
            " ({}-{})",
            zip_codes.first().unwrap_or(&"".to_string()),
            zip_codes.last().unwrap_or(&"".to_string())
        ),
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

fn deserialize_geom<'de, D>(d: D) -> Result<Option<geo::Geometry<f64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use geojson::conversion::TryInto;
    use serde::Deserialize;

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

fn serialize_bbox_as_geojson<'a, S>(
    bbox: &'a Option<Rect<f64>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use geojson::Bbox as GeojsonBbox;
    match bbox {
        Some(b) => {
            // bbox serialized as an array
            // using GeoJSON bounding box format
            // See RFC 7946: https://tools.ietf.org/html/rfc7946#section-5
            let geojson_bbox: GeojsonBbox = vec![b.min.x, b.min.y, b.max.x, b.max.y];
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
        Some(b) => Some(Rect {
            min: Coordinate { x: b[0], y: b[1] },
            max: Coordinate { x: b[2], y: b[3] },
        }),
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

#[cfg(test)]
mod test {
    use super::*;
    fn make_zone(name: &str, id: usize) -> Zone {
        make_zone_and_zip(name, id, vec![], None)
    }

    fn make_zone_and_zip(name: &str, id: usize, zips: Vec<&str>, parent: Option<usize>) -> Zone {
        Zone {
            id: ZoneIndex { index: id },
            osm_id: "".into(),
            admin_level: None,
            zone_type: Some(ZoneType::City),
            name: name.into(),
            label: "".into(),
            international_labels: BTreeMap::default(),
            international_names: BTreeMap::default(),
            center: None,
            boundary: None,
            bbox: None,
            parent: parent.map(|p| ZoneIndex { index: p }),
            tags: Tags::new(),
            center_tags: Tags::new(),
            wikidata: None,
            zip_codes: zips.iter().map(|s| s.to_string()).collect(),
            is_generated: false,
        }
    }

    #[test]
    fn simple_label_test() {
        let mut zones = vec![make_zone("toto", 0)];

        let (mslice, z) = MutableSlice::init(&mut zones, 0);
        z.compute_labels(&mslice);
        assert_eq!(z.label, "toto");
    }

    #[test]
    fn label_with_zip_and_parent() {
        let mut zones = vec![
            make_zone_and_zip("bob", 0, vec!["75020", "75021", "75022"], Some(1)),
            make_zone_and_zip("bob sur mer", 1, vec!["75"], Some(2)), // it's zip code shouldn't be used
            make_zone("bobette's land", 2),
        ];

        let (mslice, z) = MutableSlice::init(&mut zones, 0);
        z.compute_labels(&mslice);
        assert_eq!(z.label, "bob (75020-75022), bob sur mer, bobette's land");
    }

    #[test]
    fn label_with_zip_and_double_parent() {
        // we should not have any double in the label
        let mut zones = vec![
            make_zone_and_zip("bob", 0, vec!["75020"], Some(1)),
            make_zone_and_zip("bob", 1, vec![], Some(2)),
            make_zone_and_zip("bob", 2, vec![], Some(3)),
            make_zone_and_zip("bob sur mer", 3, vec!["75"], Some(4)),
            make_zone_and_zip("bob sur mer", 4, vec!["75"], Some(5)),
            make_zone("bobette's land", 5),
        ];

        let (mslice, z) = MutableSlice::init(&mut zones, 0);
        z.compute_labels(&mslice);
        assert_eq!(z.label, "bob (75020), bob sur mer, bobette's land");
    }

    #[test]
    fn label_with_zip_and_parent_named_as_zone() {
        // we should not have any consecutive double in the labl
        // but non consecutive double should not be cleaned
        let mut zones = vec![
            make_zone_and_zip("bob", 0, vec!["75020"], Some(1)),
            make_zone_and_zip("bob sur mer", 1, vec!["75"], Some(2)),
            make_zone("bob", 2),
        ];

        let (mslice, z) = MutableSlice::init(&mut zones, 0);
        z.compute_labels(&mslice);
        assert_eq!(z.label, "bob (75020), bob sur mer, bob");
    }

    #[test]
    fn test_international_names() {
        let tags = vec![
            ("another_tag", "useless"),
            ("name:fr", "bob"),
            ("name:es", "bobito"),
            ("name", "bobito"),
            ("name:a_strange_lang_name", "bibi"),
        ].into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect();

        let names = get_international_names(&tags, "bob");

        assert_eq!(
            names,
            vec![("es", "bobito"), ("a_strange_lang_name", "bibi")]
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect()
        );
    }
}
