#[macro_use]
extern crate approx;

use cosmogony::{Cosmogony, Zone, ZoneIndex, ZoneType};
use cosmogony_builder::{create_ontology, get_zones_and_stats, is_admin, is_place};
use geo_types::Point;
use osmpbfreader::OsmPbfReader;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::process::{Command, Output};

type Coord = Point<f64>;

fn launch_command_line(args: Vec<&str>) -> Output {
    let cosmogony_bin = concat!(env!("OUT_DIR"), "/../../../cosmogony");
    Command::new(cosmogony_bin)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(&args)
        .output()
        .expect("command failed")
}

#[test]
fn test_cmd_version() {
    let output = launch_command_line(vec!["--version"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("cosmogony_builder {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn test_cmd_with_json_output() {
    let out_file = concat!(env!("OUT_DIR"), "/test_cosmogony.json");
    let output = launch_command_line(vec![
        "-i",
        "./tests/data/luxembourg_filtered.osm.pbf",
        "-o",
        out_file,
    ]);
    assert!(output.status.success());

    let cosmo = cosmogony::load_cosmogony_from_file(&out_file).unwrap();
    assert_eq!(cosmo.zones.len(), 299);
}

#[test]
fn test_cmd_with_json_stream_output() {
    let out_file = concat!(env!("OUT_DIR"), "/test_cosmogony.jsonl");
    let output = launch_command_line(vec![
        "-i",
        "./tests/data/luxembourg_filtered.osm.pbf",
        "-o",
        out_file,
    ]);
    assert!(output.status.success());

    // we try also the streaming zone's reader
    let zones = cosmogony::read_zones_from_file(out_file).unwrap();
    assert_eq!(zones.count(), 299);
}

#[test]
fn test_cmd_with_json_stream_gz_output() {
    let out_file = concat!(env!("OUT_DIR"), "/test_cosmogony.jsonl.gz");
    let output = launch_command_line(vec![
        "-i",
        "./tests/data/luxembourg_filtered.osm.pbf",
        "-o",
        out_file,
    ]);
    assert!(output.status.success());

    // we try also the streaming zone's reader
    let zones = cosmogony::read_zones_from_file(out_file).unwrap();
    assert_eq!(zones.count(), 299);
}

#[test]
fn test_cmd_with_json_gz_output() {
    let out_file = concat!(env!("OUT_DIR"), "/test_cosmogony.json.gz");
    let output = launch_command_line(vec![
        "-i",
        "./tests/data/luxembourg_filtered.osm.pbf",
        "-o",
        out_file,
    ]);
    assert!(output.status.success());
    let cosmo = cosmogony::load_cosmogony_from_file(&out_file).unwrap();
    assert_eq!(cosmo.zones.len(), 299);
}

#[test]
fn test_cmd_with_unknown_format() {
    let output = launch_command_line(vec![
        "-i",
        "./tests/data/luxembourg_filtered.osm.pbf",
        "-o",
        "cosmogony.bad",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unable to detect the file format"));
}

fn create_cosmogony_for_lux() -> Cosmogony {
    let test_file = concat!(
        env!("OUT_DIR"),
        "/../../../../../tests/data/luxembourg_filtered.osm.pbf"
    );

    cosmogony_builder::build_cosmogony(test_file.into(), Some("lu".into()), true, &[])
        .expect("invalid cosmogony")
}

fn test_wrapper_for_lux_admin_levels(a_cosmogony: &Cosmogony) {
    // Ensure that all well-defined (with closed boundaries)
    // administrative zones are loaded from the sample .osm.pbf file,
    // with correct counts per admin_level.
    let level_counts = a_cosmogony.meta.stats.level_counts.clone();
    let wikidata_counts = a_cosmogony.meta.stats.wikidata_counts.clone();

    fn assert_count(counts: &BTreeMap<u32, u64>, key: u32, value: u64) {
        assert_eq!(
            *counts.get(&key).unwrap_or(&0),
            value,
            "Expected {} admins at level {}",
            value,
            key
        )
    }

    assert_count(&level_counts, 2, 1); // 1 x admin_level==2
    assert_count(&wikidata_counts, 2, 1);
    assert_count(&level_counts, 3, 0); // 0 x admin_level==3
    assert_count(&wikidata_counts, 3, 0);
    assert_count(&level_counts, 4, 0); // etc.
    assert_count(&wikidata_counts, 4, 0);
    assert_count(&level_counts, 5, 0);
    assert_count(&wikidata_counts, 5, 0);
    assert_count(&level_counts, 6, 13); // 12 cantons + 1 territory (DE-LU)
    assert_count(&wikidata_counts, 6, 13);
    assert_count(&level_counts, 7, 0);
    assert_count(&wikidata_counts, 7, 0);
    assert_count(&level_counts, 8, 105); // 104 + 1 outside LU
    assert_count(&wikidata_counts, 8, 105);
    assert_count(&level_counts, 9, 79);
    // the level 10 is not defined in the libpostal hierarchy, we should not have level 10 admins
    assert_count(&level_counts, 10, 0);
}

fn test_wrapper_for_lux_zones(a_cosmogony: &Cosmogony) {
    let zone = a_cosmogony
        .zones
        .iter()
        .find(|z| z.name == "Esch-sur-Alzette" && z.zone_type == Some(ZoneType::City))
        .unwrap();

    let bbox = zone.bbox.unwrap();
    assert_relative_eq!(bbox.min().x, 5.9432118, epsilon = 1e-8);
    assert_relative_eq!(bbox.min().y, 49.460907, epsilon = 1e-8);
    assert_relative_eq!(bbox.max().x, 6.005144, epsilon = 1e-8);
    assert_relative_eq!(bbox.max().y, 49.518616, epsilon = 1e-8);
}

#[test]
fn test_lux_cosmogony() {
    // Check some random values in the built cosmogony
    // from the sample .osm.pbf file,
    let cosmogony = create_cosmogony_for_lux();
    assert_eq!(cosmogony.meta.osm_filename, "luxembourg_filtered.osm.pbf");
    assert_eq!(cosmogony.zones.len(), 198);

    test_wrapper_for_lux_admin_levels(&cosmogony);
    test_wrapper_for_lux_zones(&cosmogony);
}

#[test]
fn test_lux_cosmogony_with_serialisation() {
    // Serialize and deserialize a built cosmogony
    // and check again the admin_level counts.
    let cosmogony = create_cosmogony_for_lux();

    let cosmogony_as_json = serde_json::to_string(&cosmogony).unwrap();
    let cosmogony_from_json: Cosmogony = serde_json::from_str(&cosmogony_as_json).unwrap();

    test_wrapper_for_lux_admin_levels(&cosmogony_from_json);
    test_wrapper_for_lux_zones(&cosmogony_from_json);
}

fn get_zone<'a>(cosmogony: &'a Cosmogony, idx: &'a ZoneIndex) -> Option<&'a Zone> {
    cosmogony.zones.iter().find(|z| z.id == *idx)
}

#[test]
fn test_lux_zone_types() {
    // Check the zone types in the built cosmogony
    let cosmogony = create_cosmogony_for_lux();
    let zone_type_counts = &cosmogony.meta.stats.zone_type_counts;
    fn assert_count(counts: &BTreeMap<String, u64>, key: &str, value: u64) {
        assert_eq!(
            *counts.get(key).unwrap_or(&0),
            value,
            "Expected {} elements of type {}",
            value,
            key
        )
    }
    assert_count(zone_type_counts, "Suburb", 55);
    assert_count(zone_type_counts, "City", 105);
    assert_count(zone_type_counts, "StateDistrict", 13);
    assert_count(zone_type_counts, "State", 0);
    assert_count(zone_type_counts, "Country", 1);
    assert_count(zone_type_counts, "None", 0); // all the zones without zone_type should be filtered

    // check Luxembourg city
    let lux = cosmogony
        .zones
        .iter()
        .find(|z| z.name == "Luxembourg" && z.zone_type == Some(ZoneType::City))
        .unwrap();
    assert_eq!(lux.osm_id, "relation:407489");
    assert_eq!(lux.admin_level, Some(8));
    assert_eq!(lux.label, "Luxembourg, Canton Luxembourg, Lëtzebuerg");
    assert_eq!(lux.country_code, Some("LU".into()));
    assert!(lux.zip_codes.is_empty());
    assert!(lux.center.is_some());
    assert_eq!(
        get_zone(&cosmogony, &lux.parent.unwrap()).unwrap().name,
        "Canton Luxembourg"
    );
    assert_eq!(lux.wikidata, Some("Q1842".into()));
    assert!(!lux.tags.is_empty());
    assert_eq!(
        lux.international_labels.get("fr"),
        Some(&"Luxembourg, Canton Luxembourg, Luxembourg".to_string())
    );
    assert_eq!(
        lux.international_labels.get("de"),
        Some(&"Luxemburg, Kanton Luxemburg, Luxemburg".to_string())
    );

    // Read names from center_tags
    assert_eq!(
        lux.international_labels.get("br"),
        Some(&"Luksembourg, Canton Luxembourg, Luksembourg".to_string())
    );

    assert!(!lux.center_tags.is_empty());
    assert_eq!(lux.center_tags.get("population"), Some(&"103641".into()));

    // check the country
    let lux = cosmogony
        .zones
        .iter()
        .find(|z| z.name == "Lëtzebuerg" && z.zone_type == Some(ZoneType::Country))
        .unwrap();
    assert_eq!(lux.osm_id, "relation:2171347");
    assert_eq!(lux.admin_level, Some(2));
    assert_eq!(lux.label, "Lëtzebuerg");
    assert_eq!(lux.country_code, Some("LU".into()));
    assert!(lux.zip_codes.is_empty());
    assert!(lux.center.is_some());
    assert_eq!(&lux.parent, &None::<ZoneIndex>);
    assert_eq!(lux.wikidata, Some("Q32".into()));
    assert!(!lux.tags.is_empty());
    assert_eq!(
        lux.international_labels.get("fr"),
        Some(&"Luxembourg".to_string())
    );
    assert_eq!(
        lux.international_labels.get("de"),
        Some(&"Luxemburg".to_string())
    );

    // Read names from label node
    assert_eq!(
        lux.international_labels.get("ak"),
        Some(&"Laksembɛg".to_string())
    );

    // Read population from label node
    assert_eq!(lux.tags.get("population"), Some(&"493500".into()));
}

#[test]
fn test_center_label() {
    let ottawa_test_file = concat!(
        env!("OUT_DIR"),
        "/../../../../../tests/data/gatineau.osm.pbf"
    );
    let cosmogony =
        cosmogony_builder::build_cosmogony(ottawa_test_file.into(), Some("ca".into()), true, &[])
            .expect("invalid cosmogony");

    let gati = cosmogony
        .zones
        .iter()
        .find(|z| z.name == "Gatineau" && z.zone_type == Some(ZoneType::City))
        .unwrap();

    assert_eq!(gati.osm_id, "relation:5356213");
    assert_eq!(gati.admin_level, Some(8));
    assert!(gati.center.is_some());
    let gati_center = gati.center.unwrap();
    assert_eq!(
        gati_center,
        Coord::new(-75.72326699999999, 45.457240999999996)
    );
}

#[test]
fn test_voronoi() {
    let ivory_test_file = concat!(
        env!("OUT_DIR"),
        "/../../../../../tests/data/ivory-coast.pbf"
    );
    let path = Path::new(&ivory_test_file);
    let file = File::open(&path).expect("no pbf file");

    let parsed_pbf = OsmPbfReader::new(file)
        .get_objs_and_deps(|o| is_admin(o) || is_place(o))
        .expect("invalid osm file");

    let (mut zones, mut stats) =
        get_zones_and_stats(&parsed_pbf).expect("get_zones_and_stats failed");

    assert_eq!(zones.len(), 118);
    create_ontology(&mut zones, &mut stats, None, false, &parsed_pbf, &[])
        .expect("create_ontology failed");
    assert_eq!(zones.len(), 4453);
}
