extern crate geo;
extern crate itertools;
#[macro_use]
extern crate log;
extern crate mimir;
extern crate mimirsbrunn;
extern crate osmpbfreader;
#[macro_use]
extern crate serde_derive;
extern crate serde_yaml;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use structopt::StructOpt;
use mimirsbrunn::osm_reader::parse_osm_pbf;
use mimirsbrunn::osm_reader::OsmPbfReader;
use itertools::Itertools;
use mimirsbrunn::boundaries::{build_boundary, make_centroid};
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::prelude::*;

mod zone;
mod admin_type;
mod model;

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

    let countries: BTreeMap<String, model::Country> = read_libpostal_yaml_folder("/data/libpostal_osm_yaml/".to_string());
    
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

pub fn read_libpostal_yaml_folder(yaml_files_folder: String) -> BTreeMap<String, model::Country>{
    let paths = fs::read_dir(yaml_files_folder).expect("The yaml directory doesn't exist.");      
    let mut countries: BTreeMap<String, model::Country> = BTreeMap::new();

    for entry in paths {
        let mut contents = String::new();
        let a_path = entry.unwrap().path();

        File::open(&a_path)
            .unwrap()
            .read_to_string(&mut contents)
            .expect("Something went wrong reading the file");

        let deserialized_country = match read_libpostal_yaml(&contents) {
            Ok(a) => a,
            Err(_) => continue,
        };

        let reference = a_path.file_name().unwrap().to_str().unwrap().get(0..2).unwrap();
        info!("{:?}: {:?}",reference, deserialized_country);
        if !deserialized_country.admin_level.contains_key("error") {
            countries.insert(reference.into(), deserialized_country);
        }
    } 
    countries
}

pub fn read_libpostal_yaml(contents: &String) -> Result<model::Country,String> {
    let mut map_error: BTreeMap<String, String> = BTreeMap::new();
    map_error.insert("error".to_string(), "error".to_string());
    let empty_country: model::Country = model::Country {admin_level: map_error};

    let deserialized_country: model::Country = match serde_yaml::from_str(&contents) {
        Ok(country) => country,
        Err(_) => {
            empty_country
        },
    };    
    
    Ok(deserialized_country)
}

#[cfg(test)]
mod tests {
    use super::read_libpostal_yaml;
    use std::collections::BTreeMap;

    #[test]
    fn test_read_libpostal_yaml() {
        let yaml_ok_1 = r#"---
        admin_level: 
            "3": "country"
            "7": "state"
            "8": "city"

        overrides:
            contained_by:
                relation:
                     "5829526":
                        admin_level:
                            "10": "suburb""#.to_string();

        let yaml_ok_2 = r#"---
        admin_level: 
            "3": "country"
            "7": "state"
            "8": "city""#.to_string();

        let yaml_ko = r#"---
        admin_level: 
            "3": "country"
            "7": "state"
            "8": "city"

            overrides:
                contained_by:
                    relation:
                        "5829526":
                            admin_level:
                                "10": "suburb""#.to_string();

        // Test for ok yaml
        let deserialized_country = read_libpostal_yaml(&yaml_ok_1).unwrap();
        assert_eq!(deserialized_country.admin_level.get(&"3".to_string()), Some(&"country".to_string()));
        let deserialized_country = read_libpostal_yaml(&yaml_ok_2).unwrap();
        assert_eq!(deserialized_country.admin_level.get(&"8".to_string()), Some(&"city".to_string()));

        // Test for ko yaml
        let deserialized_country = read_libpostal_yaml(&yaml_ko).unwrap();
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        map.insert("error".to_string(), "error".to_string());
        assert_eq!(deserialized_country.admin_level, map);
    }     
}

