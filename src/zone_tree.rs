use gst::rtree::RTree;
use std::iter::FromIterator;
use crate::utils::bbox_to_rect;
use crate::zone::{Zone, ZoneIndex};

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
