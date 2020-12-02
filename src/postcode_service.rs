use std::collections::BTreeMap;
use osmpbfreader::{OsmId, OsmObj};
use rstar::{RTree, AABB};
use crate::postcode::{PostcodeBbox, Postcode};
use failure::Error;
use crate::is_postal_code;
use geo::prelude::{BoundingRect, Area};
use geo_booleanop::boolean::BooleanOp;
use cosmogony::Zone;
use geo::{Rect, Point};

pub fn get_postcodes(
    pbf: &BTreeMap<OsmId, OsmObj>,
) -> Result<RTree<PostcodeBbox>, Error> {
    use rayon::prelude::*;

    let postcodes: Vec<PostcodeBbox> = pbf.into_par_iter()
        .filter_map(|(_, obj)| {
            if !is_postal_code(obj) {
                return None;
            }
            if let OsmObj::Relation(ref relation) = *obj {
                if let Some(postcode) = Postcode::from_osm_relation(relation, pbf) {
                    // Ignore zone without boundary polygon for the moment
                    let bbox = postcode.boundary.bounding_rect().unwrap();
                    return Some(PostcodeBbox::new(
                        postcode,
                        &bbox,
                    ));
                };
            }
            return None;
        })
        .collect();

    let tree = RTree::bulk_load(postcodes);

    Ok(tree)
}


pub fn assign_postcodes_to_zones(zones: &mut Vec<Zone>,
                                 postcodes: &RTree<PostcodeBbox>) -> () {
    use rayon::prelude::*;
    zones
        .into_par_iter()
        .for_each(|z| {
            if let Some(boundary) = z.boundary.as_ref() {
                if let Some(bbox) = z.bbox {
                    if z.zip_codes.is_empty() {
                        //info!("ZipCodes were empty for {:?}, trying to fill them", name);
                        z.zip_codes = postcodes.locate_in_envelope_intersecting(&envelope(&bbox))
                            .filter(|postcode_bbox| {
                                //info!(" - Candidate Postcode: {:?}", postcode_bbox.get_postcode().zipcode);

                                let overlap_between_postcode_and_area = BooleanOp::intersection(boundary, postcode_bbox.get_postcode().get_boundary());

                                // anteil überlappender Bereiches / Postcode: "Wieviel % des Postcodes sind von dieser Fläche befüllt"
                                let overlap_percentage_relative_to_postcode = overlap_between_postcode_and_area.unsigned_area() / postcode_bbox.get_postcode().unsigned_area();

                                //info!("   CHOSEN {} {:?}", overlap_percentage_relative_to_postcode, overlap_percentage_relative_to_postcode > 0.05);
                                // at least 5% des Postcodes müssen in der genannten Fläche liegen
                                overlap_percentage_relative_to_postcode > 0.05
                            })
                            .map(|x| x.get_postcode().zipcode.to_string())
                            .collect();
                    }
                }
            }
        });
}

fn envelope(bbox: &Rect<f64>) -> AABB<Point<f64>> {
    AABB::from_corners(bbox.min().into(), bbox.max().into())
}

