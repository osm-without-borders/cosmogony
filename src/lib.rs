extern crate failure;
#[macro_use]
extern crate log;
extern crate osmpbfreader;
#[macro_use]
extern crate serde_derive;
extern crate serde_yaml;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

mod zone;
mod admin_type;
pub mod cosmogony;

use std::fs::File;
use std::path::Path;
use cosmogony::{Cosmogony, CosmogonyMetadata, CosmogonyStats};
use osmpbfreader::{OsmObj, OsmPbfReader};
use std::collections::BTreeMap;
use std::fs;
use std::io::prelude::*;

use failure::Error;
use failure::ResultExt;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub fn is_admin(obj: &OsmObj) -> bool {
    match *obj {
        OsmObj::Relation(ref rel) => {
            rel.tags
                .get("boundary")
                .map_or(false, |v| v == "administrative")
            &&
            rel.tags.get("admin_level").is_some()
        }
        _ => false,
    }
}

pub fn get_zones_and_stats(
    pbf: &mut OsmPbfReader<File>,
) -> Result<(Vec<zone::Zone>, CosmogonyStats), Error> {
    info!("Reading pbf with geometries...");
    let objects = pbf.get_objs_and_deps(|o| is_admin(o))
        .context("invalid osm file")?;
    info!("reading pbf done.");

    let mut zones = vec![];
    let mut stats = CosmogonyStats::default();

    for obj in objects.values() {
        if !is_admin(obj) {
            continue;
        }
        if let OsmObj::Relation(ref relation) = *obj {
            if let Some(zone) = zone::Zone::from_osm_with_geom(relation, &objects) {
                // Ignore zone without boundary polygon
                if zone.boundary.is_some() {
                    stats.process(&zone);
                    zones.push(zone);
                }
            }
        }
    }

    return Ok((zones, stats));
}

pub fn get_zones_and_stats_without_geom(
    pbf: &mut OsmPbfReader<File>,
) -> Result<(Vec<zone::Zone>, CosmogonyStats), Error> {
    info!("Reading pbf without geometries...");

    let mut zones = vec![];
    let mut stats = CosmogonyStats::default();

    for obj in pbf.par_iter().map(Result::unwrap) {
        if !is_admin(&obj) {
            continue;
        }
        if let OsmObj::Relation(ref relation) = obj {
            if let Some(zone) = zone::Zone::from_osm(relation) {
                stats.process(&zone);
                zones.push(zone);
            }
        }
    }

    Ok((zones, stats))
}

pub fn build_cosmogony(pbf_path: String, with_geom: bool) -> Result<Cosmogony, Error> {
    let path = Path::new(&pbf_path);
    let file = File::open(&path).context("no pbf file")?;

    let mut parsed_pbf = OsmPbfReader::new(file);

    let (zones, stats) = if with_geom {
        get_zones_and_stats(&mut parsed_pbf)?
    } else {
        get_zones_and_stats_without_geom(&mut parsed_pbf)?
    };
    let cosmogony = Cosmogony {
        zones: zones,
        meta: CosmogonyMetadata {
            osm_filename: path.file_name()
                .and_then(|f| f.to_str())
                .map(|f| f.to_string())
                .unwrap_or("invalid file name".into()),
            stats: stats,
        },
    };
    Ok(cosmogony)
}

pub fn read_libpostal_yaml_folder(yaml_files_folder: String) -> BTreeMap<String, model::Country> {
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

        let reference = a_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .get(0..2)
            .unwrap();
        info!("{:?}: {:?}", reference, deserialized_country);
        if !deserialized_country.admin_level.contains_key("error") {
            countries.insert(reference.into(), deserialized_country);
        }
    }
    countries
}

pub fn read_libpostal_yaml(contents: &String) -> Result<model::Country, String> {
    let mut map_error: BTreeMap<String, String> = BTreeMap::new();
    map_error.insert("error".to_string(), "error".to_string());
    let empty_country: model::Country = model::Country {
        admin_level: map_error,
    };

    let deserialized_country: model::Country = match serde_yaml::from_str(&contents) {
        Ok(country) => country,
        Err(_) => empty_country,
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
        assert_eq!(
            deserialized_country.admin_level.get(&"3".to_string()),
            Some(&"country".to_string())
        );
        let deserialized_country = read_libpostal_yaml(&yaml_ok_2).unwrap();
        assert_eq!(
            deserialized_country.admin_level.get(&"8".to_string()),
            Some(&"city".to_string())
        );

        // Test for ko yaml
        let deserialized_country = read_libpostal_yaml(&yaml_ko).unwrap();
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        map.insert("error".to_string(), "error".to_string());
        assert_eq!(deserialized_country.admin_level, map);
    }
}
