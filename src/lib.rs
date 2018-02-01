extern crate failure;
#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate log;
extern crate mimirsbrunn;
extern crate osmpbfreader;
#[macro_use]
extern crate serde_derive;
extern crate serde_yaml;
extern crate structopt;

pub mod zone;
pub mod cosmogony;
pub mod zone_typer;

use std::fs::File;
use std::path::{Path, PathBuf};
use cosmogony::{Cosmogony, CosmogonyMetadata, CosmogonyStats};
use osmpbfreader::{OsmObj, OsmPbfReader};
use std::collections::BTreeMap;
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

fn get_country<'a>(_zone: &zone::Zone, country_code: &'a Option<String>) -> Result<&'a str, Error> {
    if let &Some(ref c) = country_code {
        Ok(c)
    } else {
        //TODO add a realway to find the country
        Err(failure::err_msg("Cannot find the country of the zone"))
    }
}

fn create_ontology(
    zones: &mut Vec<zone::Zone>,
    stats: &mut CosmogonyStats,
    libpostal_file_path: PathBuf,
    country_code: Option<String>,
) -> Result<(), Error> {
    let zone_typer = zone_typer::ZoneTyper::create(libpostal_file_path)?;

    for mut z in zones {
        let country = get_country(&z, &country_code)?;
        let type_res = zone_typer.get_zone_type(&z, &country);
        match type_res {
            Ok(t) => z.zone_type = Some(t),
            Err(zone_typer::ZoneTyperError::InvalidCountry(c)) => {
                info!("impossible to find {}", c);
                let zone_with_unkwown_country =
                    stats.zone_with_unkwown_country.entry(c).or_insert(0);
                *zone_with_unkwown_country += 1;
            }
            Err(zone_typer::ZoneTyperError::UnkownLevel(lvl, country)) => {
                info!("impossible to find {:?} for {}", lvl, country);
                let unhandled_admin_level_count = stats
                    .unhandled_admin_level
                    .entry(country)
                    .or_insert(BTreeMap::new())
                    .entry(lvl.unwrap_or(0))
                    .or_insert(0);
                *unhandled_admin_level_count += 1;
            }
        }
    }
    Ok(())
}

pub fn build_cosmogony(
    pbf_path: String,
    with_geom: bool,
    libpostal_file_path: PathBuf,
    country_code: Option<String>,
) -> Result<Cosmogony, Error> {
    let path = Path::new(&pbf_path);
    let file = File::open(&path).context("no pbf file")?;

    let mut parsed_pbf = OsmPbfReader::new(file);

    let (mut zones, mut stats) = if with_geom {
        get_zones_and_stats(&mut parsed_pbf)?
    } else {
        get_zones_and_stats_without_geom(&mut parsed_pbf)?
    };

    create_ontology(&mut zones, &mut stats, libpostal_file_path, country_code)?;

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
