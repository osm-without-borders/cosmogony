use crate::is_postal_code;
use crate::postcode::{Postcode, PostcodeBbox};
use cosmogony::Zone;
use geo::prelude::{Area, BoundingRect};
use geo::{Point, Rect};
use geo_booleanop::boolean::BooleanOp;
use osmpbfreader::{OsmId, OsmObj};
use rstar::{RTree, AABB};
use std::collections::BTreeMap;

pub fn assign_postcodes_to_zones(zones: &mut Vec<Zone>, pbf: &BTreeMap<OsmId, OsmObj>) {
    use rayon::prelude::*;

    info!("Starting to extract postcodes.");
    let postcodes = get_postcodes_from_pbf(pbf);
    info!("Finished extracting {} postcodes, now starting to match postcodes and zones", postcodes.size());

    zones.into_par_iter().for_each(|z| {
        if let Some(boundary) = z.boundary.as_ref() {
            if let Some(bbox) = z.bbox {
                if z.zip_codes.is_empty() {
                    //info!("ZipCodes were empty for {:?}, trying to fill them", name);
                    z.zip_codes = postcodes
                        .locate_in_envelope_intersecting(&envelope(&bbox))
                        .filter(|postcode_bbox| {
                            //info!(" - Candidate Postcode: {:?}", postcode_bbox.get_postcode().zipcode);

                            let overlap_between_postcode_and_area = BooleanOp::intersection(
                                boundary,
                                postcode_bbox.get_postcode().get_boundary(),
                            );

                            // anteil überlappender Bereiches / Postcode: "Wieviel % des Postcodes sind von dieser Fläche befüllt"
                            let overlap_percentage_relative_to_postcode =
                                overlap_between_postcode_and_area.unsigned_area()
                                    / postcode_bbox.get_postcode().unsigned_area();

                            //info!("   CHOSEN {} {:?}", overlap_percentage_relative_to_postcode, overlap_percentage_relative_to_postcode > 0.05);
                            // at least 5% des Postcodes müssen in der genannten Fläche liegen
                            overlap_percentage_relative_to_postcode > 0.05
                        })
                        .map(|x| x.get_postcode().zipcode.to_string())
                        .collect();
                    z.zip_codes.sort();
                }
            }
        }
    });
    info!("Finished matching postcodes and zones.");
}


fn get_postcodes_from_pbf(pbf: &BTreeMap<OsmId, OsmObj>) -> RTree<PostcodeBbox> {
    use rayon::prelude::*;

    let postcodes_list: Vec<PostcodeBbox> = pbf
        .into_par_iter()
        .filter_map(|(_, obj)| {
            if !is_postal_code(obj) {
                return None;
            }
            if let OsmObj::Relation(ref relation) = *obj {
                if let Some(postcode) = Postcode::from_osm_relation(relation, pbf) {
                    // Ignore zone without boundary polygon for the moment
                    let bbox = postcode.boundary.bounding_rect().unwrap();
                    return Some(PostcodeBbox::new(postcode, &bbox));
                };
            }
            None
        })
        .collect();

    RTree::bulk_load(postcodes_list)
}

fn envelope(bbox: &Rect<f64>) -> AABB<Point<f64>> {
    AABB::from_corners(bbox.min().into(), bbox.max().into())
}
