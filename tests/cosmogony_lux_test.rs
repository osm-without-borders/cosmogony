extern crate cosmogony;
extern crate serde_json;
use cosmogony::{Cosmogony, Zone, ZoneIndex, ZoneType};

use std::collections::BTreeMap;

fn create_cosmogony_for_lux() -> Cosmogony {
    let test_file = concat!(
        env!("OUT_DIR"),
        "/../../../../../tests/data/luxembourg_filtered.osm.pbf"
    );
    let cosmogony = cosmogony::build_cosmogony(
        test_file.into(),
        true,
        "./libpostal/resources/boundaries/osm".into(),
        Some("lu".into()),
    ).expect("invalid cosmogony");
    return cosmogony;
}

#[test]
fn test_lux_cosmogony() {
    // Check some random values in the built cosmogony
    // from the sample .osm.pbf file,
    let cosmogony = create_cosmogony_for_lux();
    assert_eq!(cosmogony.meta.osm_filename, "luxembourg_filtered.osm.pbf");
    assert_eq!(cosmogony.zones.len(), 201);

    assert!(
        cosmogony
            .zones
            .iter()
            .map(|zone| zone.name.to_owned())
            .any(|name| name == format!("Esch-sur-Alzette"))
    );
}

fn test_wrapper_for_lux_admin_levels(a_cosmogony: Cosmogony) {
    let level_counts = a_cosmogony.meta.stats.level_counts;
    let wikidata_counts = a_cosmogony.meta.stats.wikidata_counts;

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
    assert_count(&level_counts, 10, 3); // 2 + 1 outside LU
}

#[test]
fn test_lux_admin_levels() {
    // Ensure that all well-defined (with closed boundaries)
    // administrative zones are loaded from the sample .osm.pbf file,
    // with correct counts per admin_level.
    let cosmogony = create_cosmogony_for_lux();

    test_wrapper_for_lux_admin_levels(cosmogony);
}

#[test]
fn test_lux_admin_levels_with_serialisation() {
    // Serialize and deserialize a built cosmogony
    // and check again the admin_level counts.
    let cosmogony = create_cosmogony_for_lux();

    let cosmogony_as_json = serde_json::to_string(&cosmogony).unwrap();
    let cosmogony_from_json: Cosmogony = serde_json::from_str(&cosmogony_as_json).unwrap();

    test_wrapper_for_lux_admin_levels(cosmogony_from_json);
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
    assert_count(&zone_type_counts, "Suburb", 79);
    assert_count(&zone_type_counts, "City", 105);
    assert_count(&zone_type_counts, "StateDistrict", 13);
    assert_count(&zone_type_counts, "State", 0);
    assert_count(&zone_type_counts, "Country", 1);
    assert_count(&zone_type_counts, "None", 3);

    // check luxembroug city
    let lux = cosmogony
        .zones
        .iter()
        .find(|z| z.name == "Luxembourg" && z.zone_type == Some(ZoneType::City))
        .unwrap();
    assert_eq!(lux.osm_id, "relation:407489");
    assert_eq!(lux.admin_level, Some(8));
    assert_eq!(lux.label, "Luxembourg, Canton Luxembourg, Lëtzebuerg");
    assert!(lux.zip_codes.is_empty());
    assert!(lux.center.is_some());
    assert_eq!(
        get_zone(&cosmogony, &lux.parent.clone().unwrap())
            .unwrap()
            .name,
        "Canton Luxembourg"
    );
    assert_eq!(lux.wikidata, Some("Q1842".into()));
    assert!(!lux.tags.is_empty());

    // check the country
    let lux = cosmogony
        .zones
        .iter()
        .find(|z| z.name == "Lëtzebuerg" && z.zone_type == Some(ZoneType::Country))
        .unwrap();
    assert_eq!(lux.osm_id, "relation:2171347");
    assert_eq!(lux.admin_level, Some(2));
    assert_eq!(lux.label, "Lëtzebuerg");
    assert!(lux.zip_codes.is_empty());
    assert!(lux.center.is_some());
    assert_eq!(&lux.parent, &None::<ZoneIndex>);
    assert_eq!(lux.wikidata, Some("Q32".into()));
    assert!(!lux.tags.is_empty());
}
