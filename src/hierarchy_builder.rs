extern crate geo;

use gst::rtree::RTree;
use mutable_slice::MutableSlice;
use std::iter::FromIterator;
use utils::bbox_to_rect;
use zone::{Zone, ZoneIndex};

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
        match z.bbox {
            Some(ref b) => self.tree.insert(bbox_to_rect(b), z.id.clone()),
            None => warn!("No bbox: Cannot insert zone with osm_id {}", z.osm_id),
        }
    }

    pub fn fetch_zone_bbox(&self, z: &Zone) -> Vec<ZoneIndex> {
        match z.bbox {
            None => {
                warn!("No bbox: Cannot fetch zone with osm_id {}", z.osm_id);
                vec![]
            }
            Some(ref bbox) => self
                .tree
                .get(&bbox_to_rect(bbox))
                .into_iter()
                .map(|(_, z_idx)| z_idx.clone())
                .collect(),
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

pub fn find_inclusions(zones: &[Zone]) -> Vec<Vec<ZoneIndex>> {
    use rayon::prelude::*;
    info!("finding all the inclusions");
    let ztree: ZonesTree = zones.iter().collect();
    let mut result = vec![vec![]; zones.len()];

    zones
        .par_iter()
        .map(|z| {
            ztree
                .fetch_zone_bbox(z)
                .into_iter()
                .filter(|z_idx| z_idx != &z.id)
                .filter(|z_idx| zones[z_idx.index].contains(z))
                .collect()
        })
        .collect_into_vec(&mut result);

    result
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
pub fn build_hierarchy(zones: &mut [Zone], inclusions: Vec<Vec<ZoneIndex>>) {
    info!("building the zones's hierarchy");
    let nb_zones = zones.len();

    for i in 0..nb_zones {
        let (mslice, z) = MutableSlice::init(zones, i);

        let parent = inclusions[i]
            .iter()
            .filter_map(|c_idx| {
                let c = mslice.get(&c_idx);
                if z.can_be_child_of(c) {
                    Some(c)
                } else {
                    None
                }
            })
            .min_by_key(|z| z.zone_type);

        z.set_parent(parent.map(|z| z.id.clone()));
    }
}

#[cfg(test)]
mod test {
    use geo::boundingbox::BoundingBox;
    use geo::{LineString, MultiPolygon, Point, Polygon};
    use hierarchy_builder::{build_hierarchy, find_inclusions};
    use zone::{Zone, ZoneType};

    fn zone_factory(idx: usize, ls: LineString<f64>, zone_type: Option<ZoneType>) -> Zone {
        let p = Polygon::new(ls, vec![]);
        let mp = MultiPolygon(vec![p.clone()]);

        let mut z = Zone::default();
        z.id.index = idx;
        z.boundary = Some(mp);
        z.bbox = z.boundary.as_ref().and_then(|b| b.bbox());
        z.zone_type = zone_type;
        z
    }

    #[rustfmt::skip]
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

        let inclusions = find_inclusions(&zones);
        build_hierarchy(&mut zones, inclusions);

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

        let inclusions = find_inclusions(&zones);
        build_hierarchy(&mut zones, inclusions);

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

        let inclusions = find_inclusions(&zones);
        build_hierarchy(&mut zones, inclusions);

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

        let inclusions = find_inclusions(&zones);
        build_hierarchy(&mut zones, inclusions);

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

        let inclusions = find_inclusions(&zones);
        build_hierarchy(&mut zones, inclusions);

        assert_parent(&zones, 0, None); // z0 has no parent
        assert_parent(&zones, 1, Some(0)); // z1 parent is z0
        assert_parent(&zones, 2, Some(0)); // z2 parent is z0 even if it is contained by z1
        assert_parent(&zones, 3, Some(0)); // z3 parent is z0
    }
}
