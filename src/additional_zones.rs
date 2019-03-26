use failure::ResultExt;
use geo_types::{Coordinate, MultiPolygon, Point, Polygon, Rect};
use osmpbfreader::{OsmId, OsmObj, OsmPbfReader};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use crate::zone::{Zone, ZoneIndex, ZoneType};
use crate::zone_tree::ZonesTree;

pub fn compute_additional_cities(zones: &mut Vec<Zone>, pbf_path: &str) {
    let place_zones = read_places(pbf_path);
    info!(
        "there are {} places, we'll try to make boundaries for them",
        place_zones.len()
    );
    let zones_rtree: ZonesTree = zones.iter().filter(|z| z.is_admin()).collect();

    let new_cities: Vec<Zone> = {
        let mut candidate_parent_zones: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (parent, place) in place_zones
            .into_iter()
            .filter_map(|place| {
                get_parent(&place, &zones, &zones_rtree).map(|parent| {
                    info!(
                        "on a trouve le parent {:?} / {:?} for {:?}",
                        &parent.id, &parent.zone_type, place.id
                    );
                    (parent, place)
                })
            }).filter(|(parent, _)| {
                parent.zone_type != Some(ZoneType::City)
                    && parent.zone_type != Some(ZoneType::Suburb)
            }) {
            candidate_parent_zones
                .entry(&parent.id)
                .or_default()
                .push(place);
        }

        info!(
            "We'll compute voronois partitions for {} parent zones",
            candidate_parent_zones.len()
        );

        candidate_parent_zones
            .into_iter() //TODO into_par_iter
            .map(|(parent, places)| compute_voronoi(parent, places, &zones))
            .flatten()
            // .map_err(|e| error!("error while computing voronoi : {}", e))
            .collect()
    };

    publish_new_cities(zones, new_cities);
}

fn get_parent<'a>(place: &Zone, zones: &'a [Zone], zones_rtree: &ZonesTree) -> Option<&'a Zone> {
    zones_rtree
        .fetch_zone_bbox(&place)
        .into_iter()
        .map(|z_idx| &zones[z_idx.index])
        .filter(|z| z.contains_center(place))
        .min_by_key(|z| z.zone_type)
}

fn is_place(obj: &OsmObj) -> bool {
    match *obj {
        OsmObj::Node(ref node) => node
            .tags
            .get("place")
            .map_or(false, |v| v == "city" || v == "town"),
        _ => false,
    }
}

fn read_places(pbf_path: &str) -> Vec<Zone> {
    let path = Path::new(&pbf_path);
    let file = File::open(&path).context("no pbf file").unwrap(); //TODO remove unwrap

    let mut parsed_pbf = OsmPbfReader::new(file);
    let mut zones = vec![];

    for obj in parsed_pbf.par_iter().map(Result::unwrap) {
        if !is_place(&obj) {
            continue;
        }
        if let OsmObj::Node(ref node) = obj {
            let next_index = ZoneIndex { index: zones.len() };
            if let Some(mut zone) = Zone::from_osm_node(&node, next_index) {
                zone.zone_type = Some(ZoneType::City);
                zone.center = Some(Point::<f64>::new(node.lon(), node.lat()));
                zone.bbox = zone.center.as_ref().map(|p| Rect {
                    min: Coordinate {
                        x: p.0.x - std::f64::EPSILON,
                        y: p.0.y - std::f64::EPSILON,
                    },
                    max: Coordinate {
                        x: p.0.x + std::f64::EPSILON,
                        y: p.0.y + std::f64::EPSILON,
                    },
                });
                zones.push(zone);
            }
        }
    }
    zones
}

fn compute_voronoi(parent: &ZoneIndex, mut places: Vec<Zone>, zones: &[Zone]) -> Vec<Zone> {
    //TODO remove this collect by changing the compute_voronoi function signature
    let points: Vec<Point<_>> = places.iter().filter_map(|p| p.center).collect();

    let voronois = geos::compute_voronoi(&points, 0.).unwrap(); //TODO remove unwrap

    let calc_geom = create_forbidden_geom(&zones[parent.index]);
    for (idx, voronoi) in voronois.into_iter().enumerate() {
        let place: &mut Zone = &mut places[idx];

        place.boundary = make_boundary(voronoi, &calc_geom);
        //TODO compute the bounding box
        place.parent = Some(parent.clone());
    }
    places
        .into_iter()
        .filter(|z| z.boundary.is_some())
        .collect() // TODO remove this collect

    //TODO if possible zip it
    // for (ref mut place, voronoi) in places.iter_mut().filter(|p| p.center.is_some()).zip(voronois.into_iter()){
    // };
}

/// Create a multipolygon with the boundary of the parent zone + all the cities with real boundaries from this zone
/// This will be use to extract this geometry from the
fn create_forbidden_geom(parent_zone: &Zone) -> Option<MultiPolygon<f64>> {
    //TODO
    None
}

fn make_boundary(
    voronoi: Polygon<f64>,
    geom_to_extract: &Option<MultiPolygon<f64>>,
) -> Option<MultiPolygon<f64>> {
    // TODO extract the geom from the voronoi
    Some(MultiPolygon(vec![voronoi]))
}

fn publish_new_cities(zones: &mut Vec<Zone>, new_cities: Vec<Zone>) {
    for mut city in new_cities {
        city.id = ZoneIndex { index: zones.len() };
        zones.push(city);
    }
}
