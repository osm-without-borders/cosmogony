// extends Zones to add some capabilities
// The Zone's capabilities have been split in order to hide some functions specific to cosmogony
// and that we do not want to expose in the model

use cosmogony::{mutable_slice::MutableSlice, Coord, Zone, ZoneIndex, ZoneType, Postcode};
use geo::algorithm::bounding_rect::BoundingRect;
use geo::prelude::Contains;
use geos::Geom;
use geos::Geometry;
use itertools::Itertools;
use osm_boundaries_utils::build_boundary;
use osmpbfreader::objects::{Node, OsmId, OsmObj, Relation, Tags};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryInto;
use rstar::{RTree, AABB, RTreeObject};
use geo::{Rect, Point};
use geo::intersects::Intersects;
use crate::postcode_ext::PostcodeBbox;
use geo_booleanop::boolean::BooleanOp;

use geo_booleanop;
use geo;
use geo_types::MultiPolygon;
use geo::algorithm::area::Area;

pub trait ZoneExt {
    /// create a zone from an osm node
    fn from_osm_node(node: &Node, index: ZoneIndex) -> Option<Zone>;

    /// create a zone from an osm relation and a geometry
    fn from_osm_relation(
        relation: &Relation,
        objects: &BTreeMap<OsmId, OsmObj>,
        index: ZoneIndex,
        postcodes: &RTree<PostcodeBbox>,
    ) -> Option<Zone>;

    /// check is a zone contains another zone
    fn contains(&self, other: &Zone) -> bool;

    /// check if a zone contains another zone's center
    fn contains_center(&self, other: &Zone) -> bool;

    /// compute the labels of a zone
    fn compute_labels(&mut self, all_zones: &MutableSlice<'_>, filter_langs: &[String]);

    /// compute the names of a zone
    fn compute_names(&mut self);

    /// a zone can be a child of another zone z if:
    /// z is an admin (we don't want to have non administrative zones as parent)
    /// z's type is larger (so a State cannot have a City as parent)
    fn can_be_child_of(&self, z: &Zone) -> bool;
}

impl ZoneExt for Zone {
    fn from_osm_node(node: &Node, index: ZoneIndex) -> Option<Self> {
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

    fn from_osm_relation(
        relation: &Relation,
        objects: &BTreeMap<OsmId, OsmObj>,
        index: ZoneIndex,
        postcodes: &RTree<PostcodeBbox>,
    ) -> Option<Self> {
        use geo::centroid::Centroid;

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

        let boundary:Option<MultiPolygon<f64>> = build_boundary(relation, objects);
        let bbox = boundary.as_ref().and_then(|b| b.bounding_rect());

        let mut zip_codes: Vec<String> = zip_code
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .sorted()
            .collect();
        if let Some(boundary) = boundary.as_ref() {
            if let Some(bbox) = bbox {
                if (zip_codes.is_empty()) {
                    info!("ZipCodes were empty for {:?}, trying to fill them", name);
                    zip_codes = postcodes.locate_in_envelope_intersecting(&envelope(bbox))
                        .filter(|postcode| {
                            info!(" - Candidate Postcode: {:?}", postcode.get_postcode().zipcode);

                            let postcodeBoundary = postcode.get_postcode().get_boundary();
                            if boundary.intersects(postcodeBoundary) {
                                let x = BooleanOp::intersection(boundary, postcodeBoundary);

                                // anteil überlappender Bereiches / Postcode: "Wieviel % des Postcodes sind von dieser Fläche befüllt"
                                let percentage = x.unsigned_area() / postcodeBoundary.unsigned_area(); // TODO: cache postcodeBoundary size

                                info!("   CHOSEN {} {:?}", percentage, percentage > 0.05);
                                // at least 5% des Postcodes müssen in der genannten Fläche liegen
                                percentage > 0.05
                            } else {
                                info!("   NOT CHOSEN");
                                false
                            }

                        })
                        .map(|x| x.get_postcode().zipcode.to_string())
                        .collect();
                }
            }
        }
        let wikidata = relation.tags.get("wikidata").map(|s| s.to_string());

        let osm_id = format!("relation:{}", relation.id.0.to_string());

        let label_node = relation
            .refs
            .iter()
            .find(|r| &r.role == "label")
            .and_then(|r| objects.get(&r.member))
            .and_then(|o| o.node());

        let mut tags = relation.tags.clone();
        if let Some(node) = label_node {
            node.tags
                .iter()
                .filter(|(k, _)| k.starts_with("name:") || *k == "population")
                .for_each(|(k, v)| {
                    tags.entry(k.clone()).or_insert(v.clone());
                })
        }

        let refs = &relation.refs;
        let osm_center = refs
            .iter()
            .find(|r| &r.role == "admin_centre")
            .or_else(|| refs.iter().find(|r| &r.role == "label"))
            .and_then(|r| objects.get(&r.member))
            .and_then(|o| o.node());
        let center_tags = osm_center.map_or(Tags::new(), |n| n.tags.clone());

        let center = osm_center.map_or(
            boundary.as_ref().and_then(|b| {
                b.centroid().filter(|p| {
                    /*
                        On a broken polygon Geo may return Some(NaN,NaN) centroid.
                        It should NOT be serialized as [null,null] in the JSON output.
                    */
                    if p.x().is_nan() || p.y().is_nan() {
                        warn!("NaN in centroid {:?} for {}", p, osm_id);
                        return false;
                    }
                    true
                })
            }),
            |node| Some(Coord::new(node.lon(), node.lat())),
        );

        Some(Zone {
            id: index,
            osm_id,
            admin_level: level,
            zone_type: None,
            name: name.to_string(),
            label: "".to_string(),
            international_labels: BTreeMap::default(),
            international_names: BTreeMap::default(),
            zip_codes,
            center,
            boundary,
            bbox,
            parent: None,
            tags,
            center_tags,
            wikidata,
            is_generated: false,
        })
    }

    fn contains(&self, other: &Zone) -> bool {
        match (&self.boundary, &other.boundary) {
            (&Some(ref mpoly1), &Some(ref mpoly2)) => {
                let m_self: Result<Geometry, _> = mpoly1.try_into();
                let m_other: Result<Geometry, _> = mpoly2.try_into();

                match (&m_self, &m_other) {
                    (&Ok(ref m_self), &Ok(ref m_other)) => {
                        // In GEOS, "covers" is less strict than "contains".
                        // eg: a polygon does NOT "contain" its boundary, but "covers" it.
                        m_self.covers(m_other)
                            .map_err(|e| info!("impossible to compute geometries coverage for zone {:?}/{:?}: error {}",
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

    fn contains_center(&self, other: &Zone) -> bool {
        match (&self.boundary, &other.center) {
            (&Some(ref mpoly1), &Some(ref point)) => mpoly1.contains(point),
            _ => false,
        }
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
    fn compute_labels(&mut self, all_zones: &MutableSlice<'_>, filter_langs: &[String]) {
        let label = create_lbl(self, all_zones, |z: &Zone| z.name.clone());

        // we compute a label per language
        let it = self
            .iter_hierarchy(all_zones)
            .map(|z| z.international_names.keys())
            .flatten()
            .map(|n| n.as_str().into());
        let all_lang: BTreeSet<String> = if !filter_langs.is_empty() {
            it.filter(|n| filter_langs.iter().any(|x| x == n)).collect()
        } else {
            it.collect()
        };

        let international_labels = all_lang
            .iter()
            .map(|lang| {
                let lbl = create_lbl(self, all_zones, |z: &Zone| {
                    z.international_names.get(lang).unwrap_or(&z.name).clone()
                });
                (lang.to_string(), lbl)
            })
            .collect();

        self.international_labels = international_labels;
        self.label = label;
    }

    fn compute_names(&mut self) {
        let center_wikidata = self.center_tags.get("wikidata").map(|s| s.to_string());

        // Names from the center node can be used as additional tags, with some precautions:
        //  * for zones where the center node and and the relation itself represent the same wikidata entity
        //  * for all cities where these entities are not explicitly distinct
        if (self.wikidata.is_some() && self.wikidata == center_wikidata)
            || (self.zone_type == Some(ZoneType::City)
            && (center_wikidata.is_none() || self.wikidata.is_none()))
        {
            let center_names: Vec<_> = self
                .center_tags
                .iter()
                .filter(|(k, _)| k.starts_with("name:"))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();

            center_names.into_iter().for_each(|(k, v)| {
                self.tags.entry(k.into()).or_insert(v.into());
            })
        }
        self.international_names = get_international_names(&self.tags, &self.name);
    }

    /// a zone can be a child of another zone z if:
    /// z is an admin (we don't want to have non administrative zones as parent)
    /// z's type is larger (so a State cannot have a City as parent)
    fn can_be_child_of(&self, z: &Zone) -> bool {
        z.is_admin() && (!self.is_admin() || self.zone_type < z.zone_type)
    }
}

fn create_lbl<'a, F>(zone: &'a Zone, all_zones: &'a MutableSlice<'_>, f: F) -> String
    where
        F: Fn(&Zone) -> String,
{
    let mut hierarchy: Vec<String> = zone.iter_hierarchy(all_zones).map(f).dedup().collect();

    if let Some(ref mut zone_name) = hierarchy.first_mut() {
        zone_name.push_str(&format_zip_code(&zone.zip_codes));
    }
    hierarchy.join(", ")
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

fn envelope(bbox: Rect<f64>) -> AABB<Point<f64>> {
    AABB::from_corners(bbox.min().into(), bbox.max().into())
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

            Some((lang.as_str().into(), v.clone().into()))
        })
        .collect()
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
        z.compute_labels(&mslice, &[]);
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
        z.compute_labels(&mslice, &[]);
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
        z.compute_labels(&mslice, &[]);
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
        z.compute_labels(&mslice, &[]);
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
        ]
            .into_iter()
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
