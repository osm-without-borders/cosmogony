extern crate geo;

use std::iter::FromIterator;
use zone::{Zone, ZoneIndex};
use mutable_slice::MutableSlice;
use gst::rtree::RTree;
use geo::boundingbox::BoundingBox;
use utils::bbox_to_rect;

pub struct ZonesTree {
    tree: RTree<ZoneIndex>,
}

impl Default for ZonesTree {
    fn default() -> Self {
        ZonesTree { tree: RTree::new() }
    }
}

impl ZonesTree {
    pub fn insert_zone(&mut self, z: &Zone) {
        if let Some(ref b) = z.boundary {
            match b.bbox() {
                Some(b) => self.tree.insert(bbox_to_rect(b), z.id.clone()),
                None => warn!("No bbox: Cannot insert zone with osm_id {}", z.osm_id),
            }
        }
    }

    pub fn fetch_zone_bbox(&self, z: &Zone) -> Vec<ZoneIndex> {
        match z.boundary {
            None => vec![],
            Some(ref b) => {
                if let Some(bbox) = b.bbox() {
                    self.tree
                        .get(&bbox_to_rect(bbox))
                        .into_iter()
                        .map(|(_, z_idx)| z_idx.clone())
                        .collect()
                } else {
                    warn!("No bbox: Cannot fetch zone with osm_id {}", z.osm_id);
                    vec![]
                }
            }
        }
    }
}

impl<'a> FromIterator<&'a Zone> for ZonesTree {
    fn from_iter<I: IntoIterator<Item = &'a Zone>>(zones: I) -> Self {
        let mut ztree = ZonesTree::default();
        for z in zones {
            ztree.insert_zone(z);
        }
        ztree
    }
}

impl Zone {
    /// a zone can be a child of another zone z if:
    /// z is an admin (we don't want to have non administrative zones as parent)
    /// z's type is larger (so a State cannot have a City as parent)
    fn can_be_child_of(&self, z: &Zone) -> bool {
        z.is_admin() && (!self.is_admin() || self.zone_type < z.zone_type)
    }
}

/// Build the cosmogony hierarchy for all the zones
///
/// The hierarchy is a tree.
/// The zone parent is basically the 'smallest' admin that contains the zone
///
/// Some additional checks are done:
/// * a zone can be attached only to an administrative zone
/// * a zone must be attached to zone with a 'greater' zone_type
///     a City cannot be attached to a CityDistrict or a Suburb, it should be attached to a
///     StateDistrict, a State, a CountryRegion or a Country
pub fn build_hierarchy(zones: &mut [Zone]) {
    let ztree: ZonesTree = zones.iter().collect();
    let nb_zones = zones.len();

    for i in 0..nb_zones {
        let (mslice, z) = MutableSlice::init(zones, i);
        if z.parent.is_some() {
            continue;
        }

        let parent = ztree
            .fetch_zone_bbox(z)
            .into_iter()
            .filter(|c_idx| c_idx.index != i)
            .filter_map(|c_idx| {
                let c = mslice.get(&c_idx);
                if z.can_be_child_of(c) {
                    Some(c)
                } else {
                    None
                }
            })
            .fold(None, |smallest, candidate| {
                // we test first that the candidate's type is smaller that the smallest
                // since the contains is not cheap and if we already found a State that
                // contains `z` we can skip testing the country
                if (smallest.is_none()
                    || candidate.zone_type < smallest.and_then(|s: &Zone| s.zone_type))
                    && candidate.contains(z)
                {
                    Some(candidate)
                } else {
                    smallest
                }
            });

        z.set_parent(parent.map(|z| z.id.clone()));
    }
}

#[cfg(test)]
mod test {
    use geo::{LineString, MultiPolygon, Point, Polygon};
    use zone::{Zone, ZoneIndex, ZoneType};
    use hierarchy_builder::build_hierarchy;
    use osmpbfreader::Tags;

    fn zone_factory(idx: usize, ls: LineString<f64>, zone_type: Option<ZoneType>) -> Zone {
        let p = Polygon::new(ls, vec![]);
        let mp = MultiPolygon(vec![p.clone()]);

        Zone {
            id: ZoneIndex { index: idx },
            osm_id: "".into(),
            admin_level: None,
            zone_type: zone_type,
            name: "".into(),
            label: "".into(),
            center: None,
            boundary: Some(mp),
            parent: None,
            tags: Tags::new(),
            wikidata: None,
            zip_codes: vec![],
        }
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn create_zones() -> Vec<Zone> {
                let l0 = LineString(vec![
            Point::new(0., 0.),     //  +------+
            Point::new(0., 10.),    //  |      |
            Point::new(10., 10.),   //  |  z0  |
            Point::new(10., 0.),    //  |      |
            Point::new(0., 0.),     //  +------+
        ]);
        let z0 = zone_factory(0, l0, Some(ZoneType::Country));

        let l1 = LineString(vec![
            Point::new(1., 1.),     //  +------+
            Point::new(1., 9.),     //  |+----+|
            Point::new(9., 9.),     //  || z1 ||
            Point::new(9., 1.),     //  |+----+|
            Point::new(1., 1.),     //  +------+
        ]);
        let z1 = zone_factory(1, l1, Some(ZoneType::State));

        let l2 = LineString(vec![
            Point::new(2., 2.),     //  +------+
            Point::new(2., 8.),     //  |      |
            Point::new(8., 8.),     //  |  []<---- z2
            Point::new(8., 2.),     //  |      |
            Point::new(2., 2.),     //  +------+
        ]);
        let z2 = zone_factory(2, l2, Some(ZoneType::City));

        let l3 = LineString(vec![
            Point::new(0., 0.),     //  +------+
            Point::new(0., 5.),     //  |      |
            Point::new(10., 5.),    //  +------+
            Point::new(10., 0.),    //  |  z3  |
            Point::new(0., 0.),     //  +------+
        ]);
        let z3 = zone_factory(3, l3, Some(ZoneType::State));

        vec![z0, z1, z2, z3]
    }

    fn assert_parent(zones: &[Zone], idx: usize, expected_parent: Option<usize>) {
        match (expected_parent, zones[idx].parent.clone()) {
            (None, None) => (),
            (Some(_), None) => panic!("Zone {} should have a parent", idx),
            (None, Some(_)) => panic!("Zone {} should not have a parent", idx),
            (Some(i), Some(zi)) => assert_eq!(i, zi.index),
        }
    }

    #[test]
    fn hierarchy_test() {
        let mut zones = create_zones();

        build_hierarchy(&mut zones);

        assert_parent(&zones, 0, None); // z0 has no parent
        assert_parent(&zones, 1, Some(0)); // z1 parent is z0
        assert_parent(&zones, 2, Some(1)); // z2 parent is z1
        assert_parent(&zones, 3, Some(0)); // z3 parent is z0
    }

    #[test]
    fn hierarchy_test_parent_only_admin() {
        let mut zones = create_zones();

        // now we change the zone type of z1 to a non administrative region,
        // it should not be a parent anymore
        zones[1].zone_type = Some(ZoneType::NonAdministrative);

        build_hierarchy(&mut zones);

        assert_parent(&zones, 0, None); // z0 has no parent
        assert_parent(&zones, 1, Some(0)); // z1 parent is z0
        assert_parent(&zones, 2, Some(0)); // z2 parent is z0
        assert_parent(&zones, 3, Some(0)); // z3 parent is z0
    }

    #[test]
    fn hierarchy_test_parent_parent_respect_hierarchy_equals() {
        let mut zones = create_zones();

        // now we change the zone type of z2 to a State,
        // so it cannot have a state as parent anymore
        zones[2].zone_type = Some(ZoneType::State);

        build_hierarchy(&mut zones);

        assert_parent(&zones, 0, None); // z0 has no parent
        assert_parent(&zones, 1, Some(0)); // z1 parent is z0
        assert_parent(&zones, 2, Some(0)); // z2 parent is z0 even if it is contained by z1
        assert_parent(&zones, 3, Some(0)); // z3 parent is z0
    }

    /// A zone with a lower zone type should never be a parent to a zone with a higher zone type
    #[test]
    fn hierarchy_test_parent_parent_respect_hierarchy() {
        let mut zones = create_zones();

        // now we change the zone type of z2 to a CountryRegion,
        // so it cannot have a state as parent anymore
        zones[2].zone_type = Some(ZoneType::CountryRegion);

        build_hierarchy(&mut zones);

        assert_parent(&zones, 0, None); // z0 has no parent
        assert_parent(&zones, 1, Some(0)); // z1 parent is z0
        assert_parent(&zones, 2, Some(0)); // z2 parent is z0 even if it is contained by z1
        assert_parent(&zones, 3, Some(0)); // z3 parent is z0
    }

    /// a zone without a zone_type should not be a parent
    ///(but should be attached to an admin
    #[test]
    fn hierarchy_test_parent_parent_respect_hierarchy_no_type() {
        let mut zones = create_zones();

        // now we change the zone type of z1 to None, so it cannot be parent anymore
        zones[1].zone_type = None;

        build_hierarchy(&mut zones);

        assert_parent(&zones, 0, None); // z0 has no parent
        assert_parent(&zones, 1, Some(0)); // z1 parent is z0
        assert_parent(&zones, 2, Some(0)); // z2 parent is z0 even if it is contained by z1
        assert_parent(&zones, 3, Some(0)); // z3 parent is z0
    }
}
