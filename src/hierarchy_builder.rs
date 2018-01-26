extern crate geo;

use std::iter::FromIterator;
use zone::{Zone, ZoneIndex};
use gst::rtree::{RTree, Rect};
use ordered_float::OrderedFloat;
use geo::Bbox;
use geo::boundingbox::BoundingBox;

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

pub fn bbox_to_rect(bbox: Bbox<f64>) -> Rect {
    // rust-geo bbox algorithm returns `Bbox`,
    // while gst RTree uses `Rect` as index.
    Rect {
        xmin: OrderedFloat(down(bbox.xmin as f32)),
        xmax: OrderedFloat(up(bbox.xmax as f32)),
        ymin: OrderedFloat(down(bbox.ymin as f32)),
        ymax: OrderedFloat(up(bbox.ymax as f32)),
    }
}

pub fn build_hierarchy(zones: &mut [Zone]) {
    let ztree: ZonesTree = zones.iter().collect();
    let nb_zones = zones.len();

    for i in 0..nb_zones {
        let (mslice, z1) = MutableSlice::init(zones, i);
        if z1.parent.is_some() {
            continue;
        }

        let mut parents: Vec<_> = ztree
            .fetch_zone_bbox(z1)
            .into_iter()
            .filter(|c_idx| c_idx.index != i)
            .filter_map(|c_idx| {
                let c = mslice.get(&c_idx);
                if (c.admin_level < z1.admin_level) && c.contains(z1) {
                    Some(c)
                } else {
                    None
                }
            })
            .collect();

        // Temporary: we want to sort by admin_type
        parents.sort_by_key(|p| p.admin_level.unwrap_or(0));
        z1.set_parent(parents.last().map(|z| z.id.clone()));
    }
}

// This struct is necessary to wrap the `zones` slice
// and keep a mutable reference to a zone (and set
// its parent) while still be able to borrow another
// reference to another zone.
struct MutableSlice<'a> {
    pub right: &'a [Zone],
    pub left: &'a [Zone],
    pub idx: usize,
}

impl<'a> MutableSlice<'a> {
    pub fn init(zones: &'a mut [Zone], index: usize) -> (Self, &'a mut Zone) {
        let (left, temp) = zones.split_at_mut(index);
        let (z, right) = temp.split_at_mut(1);
        let s = Self {
            right: right,
            left: left,
            idx: index,
        };
        (s, &mut z[0])
    }

    pub fn get(&self, zindex: &ZoneIndex) -> &Zone {
        let idx = zindex.index;
        if idx < self.idx {
            return &self.left[idx];
        } else if idx == self.idx {
            panic!("Cannot retrieve middle index");
        } else {
            return &self.right[idx - self.idx - 1];
        }
    }
}

// the goal is that f in [down(f as f32) as f64, up(f as f32) as f64]
fn down(f: f32) -> f32 {
    f - (f * ::std::f32::EPSILON).abs()
}
fn up(f: f32) -> f32 {
    f + (f * ::std::f32::EPSILON).abs()
}

#[cfg(test)]
mod test {
    use geo::{LineString, MultiPolygon, Point, Polygon};
    use zone::{Zone, ZoneIndex};
    use hierarchy_builder::build_hierarchy;
    use osmpbfreader::Tags;

    fn zone_factory(idx: usize, ls: LineString<f64>, admin_level: u32) -> Zone {
        let p = Polygon::new(ls, vec![]);
        let mp = MultiPolygon(vec![p.clone()]);

        Zone {
            id: ZoneIndex { index: idx },
            osm_id: "".into(),
            admin_level: Some(admin_level),
            admin_type: None,
            name: "".into(),
            center: None,
            boundary: Some(mp),
            parent: None,
            tags: Tags::new(),
            wikidata: None,
            zip_codes: vec![],
        }
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn hierarchy_test() {
        let l0 = LineString(vec![
            Point::new(0., 0.),     //  +------+
            Point::new(0., 10.),    //  |      |
            Point::new(10., 10.),   //  |  z0  |
            Point::new(10., 0.),    //  |      |
            Point::new(0., 0.),     //  +------+
        ]);
        let z0 = zone_factory(0, l0, 2);

        let l1 = LineString(vec![
            Point::new(1., 1.),     //  +------+
            Point::new(1., 9.),     //  |+----+|
            Point::new(9., 9.),     //  || z1 ||
            Point::new(9., 1.),     //  |+----+|
            Point::new(1., 1.),     //  +------+
        ]);
        let z1 = zone_factory(1, l1, 5);

        let l2 = LineString(vec![
            Point::new(2., 2.),     //  +------+
            Point::new(2., 8.),     //  |      |
            Point::new(8., 8.),     //  |  []<---- z2
            Point::new(8., 2.),     //  |      |
            Point::new(2., 2.),     //  +------+
        ]);
        let z2 = zone_factory(2, l2, 6);

        let l3 = LineString(vec![
            Point::new(0., 0.),     //  +------+
            Point::new(0., 5.),     //  |      |
            Point::new(10., 5.),    //  +------+
            Point::new(10., 0.),    //  |  z3  |
            Point::new(0., 0.),     //  +------+
        ]);
        let z3 = zone_factory(3, l3, 3);

        let mut zones = vec![z0, z1, z2, z3];

        build_hierarchy(&mut zones);

        fn assert_parent(zones: &[Zone], idx: usize, expected_parent: Option<usize>){
            match (expected_parent, zones[idx].parent.clone()) {
                (None, None) => (),
                (Some(_), None) => panic!("Zone {} should have a parent", idx),
                (None, Some(_)) => panic!("Zone {} should not have a parent", idx),
                (Some(i), Some(zi)) => assert_eq!(i, zi.index)
            }
        }

        assert_parent(&zones, 0, None);    // z0 has no parent
        assert_parent(&zones, 1, Some(0)); // z1 parent is z0
        assert_parent(&zones, 2, Some(1)); // z2 parent is z1
        assert_parent(&zones, 3, Some(0)); // z3 parent is z0
    }
}
