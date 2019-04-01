use failure::ResultExt;
use geo_types::{Coordinate, MultiPolygon, Point, Rect};
use osmpbfreader::{OsmObj, OsmPbfReader};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use crate::zone::{Zone, ZoneIndex, ZoneType};
use crate::zone_tree::ZonesTree;
use geos::from_geo::TryInto;

pub fn compute_additional_cities(zones: &mut Vec<Zone>, pbf_path: &str) {
    let place_zones = read_places(pbf_path);
    info!(
        "there are {} places, we'll try to make boundaries for them",
        place_zones.len()
    );
    let zones_rtree: ZonesTree = zones.iter().filter(|z| z.is_admin()).collect();

    let new_cities: Vec<Zone> = {
        let mut candidate_parent_zones: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (parent, place) in place_zones.iter()
            .filter_map(|place| {
                if place.zone_type.is_none() {
                    println!("No type for {}", place.name);
                    return None
                }
                match get_parent(&place, &zones, &zones_rtree) {
                    Some(p) => {
                        if place.name == "Séguéla" {
                            println!("parent found: {} => {} {:?} {:?}", place.name, p.name, p.id, p.zone_type);
                        }
                        Some((p, place))
                    }
                    None => {
                        println!("No parent for {} / {:?}", place.name, place.zone_type);
                        None
                    }
                }
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

        println!("{:?}", candidate_parent_zones.iter().map(|(x, _)| x).collect::<Vec<_>>());

        candidate_parent_zones
            .into_iter() //TODO into_par_iter
            .filter(|(_, places)| !places.is_empty())
            .map(|(parent, mut places)| compute_voronoi(parent, &mut places, &zones, &zones_rtree))
            .flatten()
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

fn compute_voronoi(parent: &ZoneIndex, places: &[&Zone], zones: &[Zone], _zones_rtree: &ZonesTree) -> Vec<Zone> {
    let points: Vec<Point<_>> = places.iter()
                                      .filter_map(|p| p.center).collect();
    if points.len() == 1 {
        let mut place = places[0].clone();

        place.boundary = zones[parent.index].boundary.clone();
        return vec![place];
    }
    let par = zones[parent.index].boundary.clone().unwrap().try_into().unwrap();
    let voronois = geos::compute_voronoi(&points, Some(&par), 0.).unwrap();

    voronois.into_iter().enumerate().map(|(idx, voronoi)| {
        let mut place = places[idx].clone();

        place.boundary = match voronoi.try_into()
                                      .expect("conversion to geos failed")
                                      .intersection(&par)
                                      .expect("intersection failed")
                                      .try_into()
                                      .expect("conversion to geo failed") {
            geo::Geometry::Polygon(x) => Some(MultiPolygon(vec![x])),
            y => {
                let s = format!("{:?}", y);
                if let Some(x) = y.into_multi_polygon() {
                    Some(x)
                } else {
                    println!("!!! not a multipolygon... {}", s);
                    None
                }
            }
        };
        if place.name == "Séguéla" {
            println!("======> {:?}", place.boundary.is_some());
        } else {
            println!("=> {}", place.name);
        }
        place
    }).collect()
}

fn publish_new_cities(zones: &mut Vec<Zone>, new_cities: Vec<Zone>) {
    for mut city in new_cities {
        city.id = ZoneIndex { index: zones.len() };
        zones.push(city);
    }
}
