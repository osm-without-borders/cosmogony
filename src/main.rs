extern crate geo;
extern crate itertools;
#[macro_use]
extern crate log;
extern crate mimir;
extern crate mimirsbrunn;
extern crate osmpbfreader;
#[macro_use]
extern crate serde_derive;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use structopt::StructOpt;
use mimirsbrunn::osm_reader::parse_osm_pbf;
use mimirsbrunn::osm_reader::OsmPbfReader;
use itertools::Itertools;
use mimirsbrunn::boundaries::{build_boundary, make_centroid};

mod zone;
mod admin_type;

#[derive(StructOpt, Debug)]
struct Args {
    /// OSM PBF file.
    #[structopt(short = "i", long = "input")]
    input: String,
}

fn main() {
    mimir::logger_init();
    let args = Args::from_args();

    let mut parsed_pbf = parse_osm_pbf(&args.input);
    let zones = get_zones(&mut parsed_pbf);
}

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

pub fn get_zones(pbf: &mut OsmPbfReader) {
    info!("reading pbf...");
    let objects = pbf.get_objs_and_deps(|o| is_admin(o)).unwrap();
    info!("reading pbf done.");

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

            println!("{:?}", zone);
        }
    }
}
