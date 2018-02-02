extern crate geos;

use std::iter::FromIterator;
use gst::rtree::RTree;
use zone::Zone;
use utils::bbox_to_rect;
use geo::boundingbox::BoundingBox;
use self::geos::GGeom;

const COUNTRY_CODE_TAG: &str = "ISO3166-1:alpha2";

pub struct Country {
    iso: String, // ISO3166-1:alpha2 code (eg: FR, DE, US, etc.),
    zone: Zone,
    ggeom: GGeom,
}

pub struct CountryFinder {
    tree: RTree<Country>,
    empty: bool, // There is no is_empty() method in the Rtree library nor easy means to check it
}

impl Default for CountryFinder {
    fn default() -> Self {
        CountryFinder { tree: RTree::new(), empty: true }
    }
}

impl<'a> FromIterator<&'a Zone> for CountryFinder {
    fn from_iter<I: IntoIterator<Item = &'a Zone>>(zones: I) -> Self {
        let mut cfinder = CountryFinder::default();
        let mut is_empty = true;
        zones.into_iter()
            .filter_map(|z| {
                match z.tags.get(COUNTRY_CODE_TAG){
                    Some(country_code) => {
                        let code = country_code.to_lowercase();
                        info!("adding country {}", &code);
                        is_empty = false;
                        Some(Country{
                            iso: code,
                            ggeom: z.get_prepared_ggeom().unwrap(),
                            zone: z.clone(),
                        })
                    },
                    None => None
                }
            })
            .for_each(|c| {
                cfinder.insert_country(c);
            });
            cfinder.empty = is_empty;
        cfinder
    }
}

impl CountryFinder {
    pub fn insert_country(&mut self, c: Country) {
        let boundary = c.zone.clone().boundary;
        if let Some(ref b) = boundary {
            match b.bbox() {
                Some(b) => self.tree.insert(bbox_to_rect(b), c),
                None => warn!("No bbox: Cannot insert country {}", c.iso),
            }
        }
    }

    pub fn find_zone_country(&self, z: &Zone) -> Option<String> {
        match z.boundary {
            Some(ref b) => {
                if let Some(bbox) = b.bbox() {
                    let mut candidates: Vec<_> = self.tree
                        .get(&bbox_to_rect(bbox))
                        .into_iter()
                        .map(|(_, country)| country)
                        .collect();

                    candidates.sort_by_key(|c| -1 * c.zone.admin_level.unwrap_or(0) as i32);
                    candidates
                        .iter()
                        .filter(|c| c.ggeom.contains(&(&z.boundary.clone().unwrap()).into()))
                        .next()
                        .map(|c| c.iso.clone())
                    
                } else {
                    warn!("No bbox: Cannot fetch country of zone {}", z.osm_id);
                    None
                }
            },
            None => None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.empty
    }
}
