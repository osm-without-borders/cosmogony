use geo_types::{MultiPolygon};
use osmpbfreader::{Relation, OsmId, OsmObj};
use std::collections::BTreeMap;
use osm_boundaries_utils::build_boundary;
use geo::algorithm::area::Area;
use rstar::{AABB, RTreeObject};
use geo::{Point, Rect};

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

    /// create a zone from an osm relation and a geometry
    pub fn from_osm_relation(
        relation: &Relation,
        objects: &BTreeMap<OsmId, OsmObj>,
    ) -> Option<Postcode> {
// Skip postcode withjout postcode
        let zipcode = match relation.tags.get("postal_code") {
            Some(val) => val,
            None => {
                debug!(
                    "relation/{}: postcode region without name, skipped",
                    relation.id.0
                );
                ""
            }
        };

        let osm_id = format!("relation:{}", relation.id.0.to_string());

        let boundary = build_boundary(relation, objects);

        boundary.map(|boundary| {
            let area = boundary.unsigned_area();
            Postcode {
                osm_id,
                zipcode: zipcode.to_string(),
                boundary,
                area
            }
        })
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


#[derive(Debug)]
pub struct PostcodeBbox {
    postcode: Postcode,
    bbox: AABB<Point<f64>>,
}

impl PostcodeBbox {
    pub fn new(postcode: Postcode, bbox: &Rect<f64>) -> Self {
        PostcodeBbox {
            postcode,
            bbox: envelope(&bbox),
        }
    }

    pub fn get_postcode(&self) -> &Postcode {
        return &self.postcode;
    }
}


impl RTreeObject for PostcodeBbox {
    type Envelope = AABB<Point<f64>>;
    fn envelope(&self) -> Self::Envelope {
        self.bbox
    }
}


fn envelope(bbox: &Rect<f64>) -> AABB<Point<f64>> {
    AABB::from_corners(bbox.min().into(), bbox.max().into())
}



impl Postcode {}
