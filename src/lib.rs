#[macro_use]
extern crate include_dir;
extern crate failure;
extern crate geo;
extern crate geo_types;
#[macro_use]
extern crate log;
extern crate ordered_float;
extern crate osm_boundaries_utils;
extern crate osmpbfreader;
extern crate serde;
extern crate serde_derive;
extern crate regex;
extern crate serde_yaml;
extern crate structopt;
extern crate lazy_static;
extern crate geos;
extern crate rayon;

mod additional_zones;
pub mod cosmogony;
mod country_finder;
pub mod file_format;
mod hierarchy_builder;
mod mutable_slice;
pub mod zone;
pub mod zone_typer;

pub use crate::cosmogony::{Cosmogony, CosmogonyMetadata, CosmogonyStats};
use crate::country_finder::CountryFinder;
use crate::file_format::OutputFormat;
use crate::hierarchy_builder::{build_hierarchy, find_inclusions};
use crate::mutable_slice::MutableSlice;
use failure::Error;
use failure::ResultExt;
use log::{debug, info};
use osmpbfreader::{OsmObj, OsmPbfReader};
use additional_zones::compute_additional_cities;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

pub use crate::zone::{Zone, ZoneIndex, ZoneType};

#[rustfmt::skip]
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
    let objects = pbf
        .get_objs_and_deps(|o| is_admin(o))
        .context("invalid osm file")?;
    info!("reading pbf done.");

    let mut zones = vec![];
    let stats = CosmogonyStats::default();

    for obj in objects.values() {
        if !is_admin(obj) {
            continue;
        }
        if let OsmObj::Relation(ref relation) = *obj {
            let next_index = ZoneIndex { index: zones.len() };
            if let Some(zone) = zone::Zone::from_osm_with_geom(relation, &objects, next_index) {
                // Ignore zone without boundary polygon for the moment
                if zone.boundary.is_some() {
                    zones.push(zone);
                }
            };
        }
    }

    return Ok((zones, stats));
}

pub fn get_zones_and_stats_without_geom(
    pbf: &mut OsmPbfReader<File>,
) -> Result<(Vec<zone::Zone>, CosmogonyStats), Error> {
    info!("Reading pbf without geometries...");

    let mut zones = vec![];
    let stats = CosmogonyStats::default();

    for obj in pbf.par_iter().map(Result::unwrap) {
        if !is_admin(&obj) {
            continue;
        }
        if let OsmObj::Relation(ref relation) = obj {
            let next_index = ZoneIndex { index: zones.len() };
            if let Some(zone) = zone::Zone::from_osm(relation, &BTreeMap::default(), next_index) {
                zones.push(zone);
            }
        }
    }

    Ok((zones, stats))
}

fn get_country_code<'a>(
    country_finder: &'a CountryFinder,
    zone: &zone::Zone,
    country_code: &'a Option<String>,
    inclusions: &Vec<ZoneIndex>,
) -> Option<String> {
    if let Some(ref c) = *country_code {
        Some(c.to_uppercase())
    } else {
        country_finder.find_zone_country(&zone, &inclusions)
    }
}

fn type_zones(
    zones: &mut [zone::Zone],
    stats: &mut CosmogonyStats,
    country_code: Option<String>,
    inclusions: &Vec<Vec<ZoneIndex>>,
) -> Result<(), Error> {
    use rayon::prelude::*;
    info!("reading libpostal's rules");
    let zone_typer = zone_typer::ZoneTyper::new()?;

    info!("creating a countrys rtree");
    let country_finder: CountryFinder = CountryFinder::init(&zones, &zone_typer);
    if country_code.is_none() && country_finder.is_empty() {
        return Err(failure::err_msg(
            "no country_code has been provided and no country have been found, we won't be able to make a cosmogony",
        ));
    }

    info!("typing zones");
    // We type all the zones in parallele
    // To not mutate the zones while doing it
    // (the borrow checker would not be happy since we also need to access to the zone's vector
    // to be able to transform the ZoneIndex to a zone)
    // we collect all the types in a Vector, and assign the zone's zone_type as a post process
    let zones_type: Vec<_> = zones
        .par_iter()
        .map(|z| {
            get_country_code(&country_finder, &z, &country_code, &inclusions[z.id.index])
                .map(|c| zone_typer.get_zone_type(&z, &c, &inclusions[z.id.index], zones))
        }).collect();

    zones
        .iter_mut()
        .zip(zones_type.into_iter())
        .for_each(|(z, zone_type)| match zone_type {
            None => {
                info!(
                    "impossible to find a country for {} ({}), skipping",
                    z.osm_id, z.name
                );
                stats.zone_without_country += 1;
            }
            Some(Ok(t)) => z.zone_type = Some(t),
            Some(Err(zone_typer::ZoneTyperError::InvalidCountry(c))) => {
                info!("impossible to find rules for country {}", c);
                *stats.zone_with_unkwown_country_rules.entry(c).or_insert(0) += 1;
            }
            Some(Err(zone_typer::ZoneTyperError::UnkownLevel(lvl, country))) => {
                debug!(
                    "impossible to find a rule for level {:?} for country {}",
                    lvl, country
                );
                *stats
                    .unhandled_admin_level
                    .entry(country)
                    .or_insert_with(BTreeMap::new)
                    .entry(lvl.unwrap_or(0))
                    .or_insert(0) += 1;
            }
        });

    Ok(())
}

fn compute_labels(zones: &mut [Zone]) {
    info!("computing all zones's label");
    let nb_zones = zones.len();
    for i in 0..nb_zones {
        let (mslice, z) = MutableSlice::init(zones, i);
        z.compute_labels(&mslice);
    }
}

// we don't want to keep zone's without zone_type (but the zone_type could be ZoneType::NonAdministrative)
fn clean_untagged_zones(zones: &mut Vec<zone::Zone>) {
    info!("cleaning untagged zones");
    let nb_zones = zones.len();
    zones.retain(|z| z.zone_type.is_some());
    info!("{} zones cleaned", (nb_zones - zones.len()));
}

fn create_ontology(
    zones: &mut Vec<zone::Zone>,
    stats: &mut CosmogonyStats,
    country_code: Option<String>,
    pbf_path: &str,
) -> Result<(), Error> {
    info!("creating ontology for {} zones", zones.len());
    let (inclusions, ztree) = find_inclusions(zones);

    type_zones(zones, stats, country_code, &inclusions)?;

    build_hierarchy(zones, inclusions);

    compute_additional_cities(zones, pbf_path, ztree);

    zones.iter_mut().for_each(|z| z.compute_names());

    compute_labels(zones);

    // we remove the useless zones from cosmogony
    // WARNING: this invalidate the different indexes  (we can no longer lookup a Zone by it's id in the zones's vector)
    // this should be removed later on (and switch to a map by osm_id ?) as it's not elegant,
    // but for the moment it'll do
    clean_untagged_zones(zones);

    Ok(())
}

pub fn build_cosmogony(
    pbf_path: String,
    with_geom: bool,
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

    create_ontology(
        &mut zones,
        &mut stats,
        country_code,
        &pbf_path,
    )?;

    stats.compute(&zones);

    let cosmogony = Cosmogony {
        zones,
        meta: CosmogonyMetadata {
            osm_filename: path
                .file_name()
                .and_then(|f| f.to_str())
                .map(|f| f.to_string())
                .unwrap_or_else(|| "invalid file name".into()),
            stats,
        },
    };
    Ok(cosmogony)
}

/// Stream Cosmogony's Zone from a Reader
pub fn read_zones(
    reader: impl std::io::BufRead,
) -> impl std::iter::Iterator<Item = Result<Zone, Error>> {
    reader
        .lines()
        .map(|l| l.map_err(|e| failure::err_msg(e.to_string())))
        .map(|l| {
            l.and_then(|l| serde_json::from_str(&l).map_err(|e| failure::err_msg(e.to_string())))
        })
}

fn from_json_stream(reader: impl std::io::BufRead) -> Result<Cosmogony, Error> {
    let zones = read_zones(reader).collect::<Result<_, _>>()?;

    Ok(Cosmogony {
        zones,
        ..Default::default()
    })
}

/// Load a cosmogony from a file
pub fn load_cosmogony_from_file(input: &str) -> Result<Cosmogony, Error> {
    let format = OutputFormat::from_filename(input)?;
    let f = std::fs::File::open(&input)?;
    let f = std::io::BufReader::new(f);
    load_cosmogony(f, format)
}

/// Return an iterator on the zones
/// if the input file is a jsonstream, the zones are streamed
/// if the input file is a json, the whole cosmogony is loaded
pub fn read_zones_from_file(
    input: &str,
) -> Result<Box<dyn std::iter::Iterator<Item = Result<Zone, Error>>>, Error> {
    let format = OutputFormat::from_filename(input)?;
    let f = std::fs::File::open(&input)?;
    let f = std::io::BufReader::new(f);
    match format {
        OutputFormat::JsonGz | OutputFormat::Json => {
            let cosmo = load_cosmogony(f, format)?;
            Ok(Box::new(cosmo.zones.into_iter().map(|z| Ok(z))))
        }
        OutputFormat::JsonStream => Ok(Box::new(read_zones(f))),
        OutputFormat::JsonStreamGz => {
            let r = flate2::bufread::GzDecoder::new(f);
            let r = std::io::BufReader::new(r);
            Ok(Box::new(read_zones(r)))
        }
    }
}

/// Load a cosmogony from a reader and a file_format
pub fn load_cosmogony(
    reader: impl std::io::BufRead,
    format: OutputFormat,
) -> Result<Cosmogony, Error> {
    match format {
        OutputFormat::JsonGz => {
            let r = flate2::read::GzDecoder::new(reader);
            serde_json::from_reader(r).map_err(|e| failure::err_msg(e.to_string()))
        }
        OutputFormat::Json => {
            serde_json::from_reader(reader).map_err(|e| failure::err_msg(e.to_string()))
        }
        OutputFormat::JsonStream => from_json_stream(reader),
        OutputFormat::JsonStreamGz => {
            let r = flate2::bufread::GzDecoder::new(reader);
            let r = std::io::BufReader::new(r);
            from_json_stream(r)
        }
    }
}
