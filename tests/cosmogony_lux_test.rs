extern crate cosmogony;

use std::collections::BTreeMap;

#[test]
fn read_lux_admin_levels() {
    // Ensure that all well-defined (with closed boundaries)
    // administrative zones are loaded from the sample .osm.pbf file,
    // with correct counts per admin_level.

    let test_file = concat!(
        env!("OUT_DIR"),
        "/../../../../../tests/data/luxembourg_filtered.osm.pbf"
    );
    let cosmogony = cosmogony::build_cosmogony(test_file.into()).expect("invalid cosmology");
    assert_eq!(cosmogony.meta.osm_filename, "luxembourg_filtered.osm.pbf");

    let level_counts = cosmogony.meta.stats.level_counts;

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
    assert_count(&level_counts, 3, 0); // 0 x admin_level==3
    assert_count(&level_counts, 4, 0); // etc.
    assert_count(&level_counts, 5, 0);
    assert_count(&level_counts, 6, 13); // 12 cantons + 1 territory (DE-LU)
    assert_count(&level_counts, 7, 0);
    assert_count(&level_counts, 8, 105); // 104 + 1 outside LU
    assert_count(&level_counts, 9, 79);
    assert_count(&level_counts, 10, 3); // 2 + 1 outside LU

    assert_eq!(cosmogony.zones.len(), 201);
}
