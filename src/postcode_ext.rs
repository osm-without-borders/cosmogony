// extends Zones to add some capabilities
// The Zone's capabilities have been split in order to hide some functions specific to cosmogony
// and that we do not want to expose in the model

use cosmogony::{mutable_slice::MutableSlice, Coord, Zone, ZoneIndex, ZoneType, Postcode};
use osm_boundaries_utils::build_boundary;
use osmpbfreader::objects::{OsmId, OsmObj, Relation};
use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryInto;
use rstar::{RTreeObject, AABB};
use geo::{Point, Rect};


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

pub trait PostcodeExt {
    /// create a zone from an osm relation and a geometry
    fn from_osm_relation(
        relation: &Relation,
        objects: &BTreeMap<OsmId, OsmObj>,
    ) -> Option<Postcode>;
}

impl PostcodeExt for Postcode {

    fn from_osm_relation(
        relation: &Relation,
        objects: &BTreeMap<OsmId, OsmObj>,
    ) -> Option<Self> {
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

        Some(Postcode {
            osm_id,
            zipcode: zipcode.to_string(),
            boundary,
        })
    }
}
