extern crate geo;
extern crate geojson;
extern crate itertools;
#[macro_use]
extern crate log;
extern crate mimir;
extern crate mimirsbrunn;
extern crate osmpbfreader;
extern crate serde;
#[macro_use]
extern crate serde_derive;

mod zone;
mod admin_type;
pub mod cosmogony;

use std::fs::File;
use std::path::Path;
use mimirsbrunn::osm_reader::OsmPbfReader;
use itertools::Itertools;
use mimirsbrunn::boundaries::{build_boundary, make_centroid};
use cosmogony::{Cosmogony, CosmogonyMetadata, CosmogonyStats};

#[cfg_attr(rustfmt, rustfmt_skip)]
pub fn is_admin(obj: &osmpbfreader::OsmObj) -> bool {
    match *obj {
        osmpbfreader::OsmObj::Relation(ref rel) => {
            rel.tags
                .get("boundary")
                .map_or(false, |v| v == "administrative")
            &&
            rel.tags.get("admin_level").is_some()
        }
        _ => false,
    }
}

pub fn get_zones_and_stats(pbf: &mut OsmPbfReader) -> (Vec<zone::Zone>, CosmogonyStats) {
    info!("reading pbf...");
    let objects = pbf.get_objs_and_deps(|o| is_admin(o)).unwrap();
    info!("reading pbf done.");

    let mut zones = vec![];
    let mut stats = CosmogonyStats::default();

    for obj in objects.values() {
        if !is_admin(obj) {
            continue;
        }
        if let osmpbfreader::OsmObj::Relation(ref relation) = *obj {
            let level = relation
                .tags
                .get("admin_level")
                .and_then(|s| s.parse().ok());

            // Skip administrative region without name
            let name = match relation.tags.get("name") {
                Some(val) => val,
                None => {
                    warn!(
                        "relation/{}: adminstrative region without name, skipped",
                        relation.id.0
                    );
                    continue;
                }
            };

            let coord_center = relation
                .refs
                .iter()
                .find(|r| r.role == "admin_centre")
                .and_then(|r| objects.get(&r.member))
                .and_then(|o| o.node())
                .map(|node| mimir::Coord::new(node.lat(), node.lon()));
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
            let boundary = build_boundary(relation, &objects);
            let zone = zone::Zone {
                id: relation.id.0.to_string(),
                admin_level: level,
                admin_type: None,
                name: name.to_string(),
                zip_codes: zip_codes,
                center: coord_center.unwrap_or_else(|| make_centroid(&boundary)),
                boundary: boundary,
                parent: None,
                tags: vec![],
            };

            // Ignore zone without boundary polygon
            if zone.boundary.is_none() {
                continue;
            }

            zone.admin_level.map(|level| {
                let count = stats.level_counts.entry(level).or_insert(0);
                *count += 1;
            });
            zones.push(zone);
        }
    }

    return (zones, stats);
}

pub fn build_cosmogony(pbf_path: String) -> Cosmogony {
    let path = Path::new(&pbf_path);
    let file = File::open(&path).unwrap();

    let mut parsed_pbf = osmpbfreader::OsmPbfReader::new(file);

    let (zones, stats) = get_zones_and_stats(&mut parsed_pbf);
    let cosmogony = Cosmogony {
        zones: zones,
        meta: CosmogonyMetadata {
            osm_filename: path.file_name().unwrap().to_str().unwrap().to_string(),
            stats: stats,
        },
    };
    cosmogony
}
