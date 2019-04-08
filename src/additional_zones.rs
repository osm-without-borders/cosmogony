use geo_types::{Coordinate, MultiPolygon, Point, Rect};
use osmpbfreader::{OsmObj, OsmPbfReader};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use crate::zone::{Zone, ZoneIndex, ZoneType};
use crate::hierarchy_builder::ZonesTree;
use geos::from_geo::TryInto;
use geos::GGeom;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

struct ZoneWithGeos<'a> {
    zone: &'a Zone,
    geos: GGeom<'a>,
}

unsafe impl<'a> Send for ZoneWithGeos<'a> {}
unsafe impl<'a> Sync for ZoneWithGeos<'a> {}

impl<'a> ZoneWithGeos<'a> {
    fn new(zone: &'a Zone) -> ZoneWithGeos<'a> {
        ZoneWithGeos {
            zone,
            geos: zone.boundary.as_ref().unwrap().try_into().expect("failed to convert to geos"),
        }
    }
}

pub fn compute_additional_cities(zones: &mut Vec<Zone>, pbf_path: &str, zones_rtree: ZonesTree) {
    let place_zones = read_places(pbf_path);
    info!(
        "there are {} places, we'll try to make boundaries for them",
        place_zones.len()
    );

    let new_cities: Vec<Zone> = {
        let mut candidate_parent_zones: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (parent, place) in place_zones.iter()
            .filter_map(|place| {
                if place.zone_type.is_none() {
                    return None
                }
                get_parent(&place, &zones, &zones_rtree).map(|p| (p, place))
            }).filter(|(p, _)| {
                p.zone_type.as_ref().map(|x| *x > ZoneType::City).unwrap_or_else(|| false)
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

        let towns = zones.iter()
                         .filter(|x| x.zone_type == Some(ZoneType::City) &&
                                     x.boundary.is_some() &&
                                     !x.name.is_empty())
                         .map(|x| ZoneWithGeos::new(x))
                         .collect::<Vec<_>>();

        candidate_parent_zones
            .into_iter()
            .filter(|(_, places)| !places.is_empty())
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|(parent, mut places)| compute_voronoi(parent, &mut places, &zones, &towns))
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
            .map_or(false, |v| v == "city" || v == "town" || v == "village"),
        _ => false,
    }
}

fn read_places(pbf_path: &str) -> Vec<Zone> {
    let path = Path::new(&pbf_path);
    let file = File::open(&path).expect("no pbf file");

    let mut parsed_pbf = OsmPbfReader::new(file);
    let mut zones = vec![];

    for obj in parsed_pbf.par_iter().map(Result::unwrap) {
        if !is_place(&obj) {
            continue;
        }
        if let OsmObj::Node(ref node) = obj {
            let next_index = ZoneIndex { index: zones.len() };
            if let Some(mut zone) = Zone::from_osm_node(&node, next_index) {
                if zone.name.is_empty() {
                    continue;
                }
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
                zone.is_generated = true;
                zones.push(zone);
            }
        }
    }
    zones
}

fn convert_to_geo(geom: GGeom) -> Option<MultiPolygon<f64>> {
    match geom.try_into().expect("conversion to geo failed") {
        geo::Geometry::Polygon(x) => Some(MultiPolygon(vec![x])),
        y => {
            if let Some(x) = y.into_multi_polygon() {
                Some(x)
            } else {
                None
            }
        }
    }
}

fn extrude_existing_town<'a, 'b: 'a, T: IntoIterator<Item = &'a ZoneWithGeos<'b>>>(zone: &mut Zone, towns: T) {
    if let Some(ref mut boundary) = zone.boundary {
        let mut updates = 0;
        let mut g_boundary = boundary.try_into().expect("failed to convert to geos");
        for town in towns {
            if g_boundary.intersects(&town.geos).unwrap_or_else(|_| false) {
                if let Ok(b) = g_boundary.difference(&town.geos) {
                    updates += 1;
                    g_boundary = b;
                }
            }
        }
        if updates > 0 {
            if let Some(g) = convert_to_geo(g_boundary) {
                *boundary = g;
            }
        }
    }
}

fn compute_voronoi(parent: &ZoneIndex, places: &[&Zone], zones: &[Zone], towns: &[ZoneWithGeos]) -> Vec<Zone> {
    let points: Vec<Point<_>> = places.iter()
                                      .filter_map(|p| p.center)
                                      .collect();
    if points.len() == 1 {
        let mut place = places[0].clone();
        let parent = &zones[parent.index];

        if parent.zone_type == Some(ZoneType::Country) {
            // If the parent is the country, we don't want to have a city with the size of a country
            // so we generated a (way) smaller shape.
            place.boundary = Some(convert_to_geo(
                                    place.center.as_ref()
                                                .map(|x| x.try_into()
                                                          .expect("failed to convert point"))
                                                .unwrap()
                                                .buffer(0.01, 2)
                                                .expect("Failed to create a buffer"))
                                  .expect("failed to convert to geo"));
        } else {
            place.boundary = parent.boundary.clone();
        }
        let parent = place.parent;
        extrude_existing_town(&mut place, towns.iter().filter(|t| t.zone.parent == parent));
        return vec![place];
    }
    let par = zones[parent.index].boundary.as_ref().unwrap().try_into().unwrap();
    let voronois = geos::compute_voronoi(&points, Some(&par), 0.).unwrap();

    voronois.into_iter().enumerate().map(|(idx, voronoi)| {
        let mut place = places[idx].clone();
        let parent = place.parent;

        let s = voronoi.try_into()
                       .expect("conversion to geos failed")
                       .intersection(&par)
                       .expect("intersection failed");
        place.boundary = convert_to_geo(s);
        extrude_existing_town(&mut place, towns.iter().filter(|t| t.zone.parent == parent));
        place
    }).collect()
}

fn publish_new_cities(zones: &mut Vec<Zone>, new_cities: Vec<Zone>) {
    for mut city in new_cities {
        city.id = ZoneIndex { index: zones.len() };
        zones.push(city);
    }
}
