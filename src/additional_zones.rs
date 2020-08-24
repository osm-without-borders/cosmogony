use crate::hierarchy_builder::ZonesTree;
use crate::is_place;
use cosmogony::{Zone, ZoneIndex, ZoneType};
use geo::prelude::BoundingRect;
use geo_types::{Coordinate, MultiPolygon, Point, Rect};
use geos::{Geom, Geometry};
use osmpbfreader::{OsmId, OsmObj};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::convert::TryInto;

use crate::zone_ext::ZoneExt;

#[allow(dead_code)]
struct ZoneWithGeos<'a> {
    zone: &'a Zone,
    geos: Geometry<'a>,
}

unsafe impl<'a> Send for ZoneWithGeos<'a> {}
unsafe impl<'a> Sync for ZoneWithGeos<'a> {}

impl<'a> ZoneWithGeos<'a> {
    fn new(zone: &'a Zone) -> Option<ZoneWithGeos<'a>> {
        if let Some(ref b) = zone.boundary {
            Some(ZoneWithGeos {
                zone,
                geos: match b.try_into() {
                    Ok(g) => g,
                    Err(e) => {
                        println!(
                            "ZoneWithGeos::new failed to convert to geos zone {}: {}",
                            zone.osm_id, e
                        );
                        return None;
                    }
                },
            })
        } else {
            None
        }
    }
}

fn is_city(zone: &Zone) -> bool {
    zone.zone_type == Some(ZoneType::City) && zone.boundary.is_some() && !zone.name.is_empty()
}

pub fn compute_additional_cities(
    zones: &mut Vec<Zone>,
    parsed_pbf: &BTreeMap<OsmId, OsmObj>,
    zones_rtree: ZonesTree,
) {
    let place_zones = read_places(parsed_pbf);
    info!(
        "there are {} places, we'll try to make boundaries for them",
        place_zones.len()
    );

    let mut z_idx_to_place_idx = HashMap::new();
    let mut candidate_parent_zones: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (parent, place) in place_zones
        .iter()
        .filter_map(|place| {
            if place.zone_type.is_none() {
                return None;
            }
            get_parent(&place, &zones, &zones_rtree).map(|p| (p, place))
        })
        .filter(|(p, place)| {
            p.zone_type
                .as_ref()
                .map(|x| {
                    if *x == ZoneType::Country {
                        info!(
                            "Ignoring place with id {} and country {} as parent",
                            place.osm_id, p.osm_id
                        );
                    }
                    *x > ZoneType::City && *x < ZoneType::Country
                })
                .unwrap_or_else(|| false)
        })
    {
        candidate_parent_zones
            .entry(&parent.id)
            .or_default()
            .push(place);
    }

    info!(
        "We'll compute voronois partitions for {} parent zones",
        candidate_parent_zones.len()
    );

    let mut current_length = 0;
    let new_cities: Vec<Vec<Zone>> = {
        let relation_cities_with_geos = zones
            .iter()
            .enumerate()
            .filter(|(_, x)| is_city(x))
            .filter_map(|(pos, x)| {
                z_idx_to_place_idx.insert(pos, current_length);
                current_length += 1;
                ZoneWithGeos::new(x)
            })
            .collect::<Vec<_>>();

        candidate_parent_zones
            .into_iter()
            .filter(|(_, places)| !places.is_empty())
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|(parent, places)| {
                compute_voronoi(
                    parent,
                    &places,
                    &zones,
                    &relation_cities_with_geos,
                    &zones_rtree,
                    &z_idx_to_place_idx,
                )
            })
            .collect()
    };
    for cities in new_cities.into_iter() {
        publish_new_cities(zones, cities);
    }
}

fn get_parent<'a>(place: &Zone, zones: &'a [Zone], zones_rtree: &ZonesTree) -> Option<&'a Zone> {
    zones_rtree
        .fetch_zone_bbox(&place)
        .into_iter()
        .map(|z_idx| &zones[z_idx.index])
        .filter(|z| z.contains_center(place) && z.zone_type.is_some())
        .min_by_key(|z| z.zone_type)
}

fn read_places(parsed_pbf: &BTreeMap<OsmId, OsmObj>) -> Vec<Zone> {
    parsed_pbf
        .values()
        .enumerate()
        .filter_map(|(index, obj)| {
            if !is_place(&obj) {
                return None;
            }
            if let OsmObj::Node(ref node) = obj {
                let next_index = ZoneIndex { index };
                if let Some(mut zone) = Zone::from_osm_node(&node, next_index) {
                    if zone.name.is_empty() {
                        return None;
                    }
                    zone.zone_type = Some(ZoneType::City);
                    zone.center = Some(Point::<f64>::new(node.lon(), node.lat()));
                    zone.bbox = zone.center.as_ref().map(|p| {
                        Rect::new(
                            Coordinate {
                                x: p.0.x - std::f64::EPSILON,
                                y: p.0.y - std::f64::EPSILON,
                            }, // min
                            Coordinate {
                                x: p.0.x + std::f64::EPSILON,
                                y: p.0.y + std::f64::EPSILON,
                            }, // max
                        )
                    });
                    zone.is_generated = true;
                    return Some(zone);
                }
            }
            None
        })
        .collect()
}

fn convert_to_geo(geom: Geometry<'_>) -> Option<MultiPolygon<f64>> {
    match match geom.try_into() {
        Ok(c) => c,
        Err(e) => {
            println!("convert_to_geo: conversion to geo failed: {}", e);
            return None;
        }
    } {
        geo::Geometry::Polygon(x) => Some(MultiPolygon(vec![x])),
        y => {
            if let Ok(x) = y.try_into() {
                Some(x)
            } else {
                None
            }
        }
    }
}

// Extrude all common parts between `zone` and the given `towns`. If an error occurs during the
// process, it'll return `false`.
fn extrude_existing_town(zone: &mut Zone, towns: &[&ZoneWithGeos<'_>]) -> bool {
    if towns.is_empty() {
        return true;
    }
    if let Some(ref boundary) = zone.boundary {
        let mut updates = 0;
        let mut g_boundary = match geos::Geometry::try_from(boundary) {
            Ok(b) => b,
            Err(e) => {
                println!(
                    "extrude_existing_town: failed to convert to geos for zone {}: {}",
                    zone.osm_id, e
                );
                return false;
            }
        };
        for town in towns {
            if g_boundary.intersects(&town.geos).unwrap_or_else(|_| false) {
                match g_boundary.difference(&town.geos) {
                    Ok(b) => {
                        updates += 1;
                        g_boundary = b;
                    }
                    Err(e) => {
                        println!(
                            "extrude_existing_town: difference failed for {}: {:?}",
                            zone.osm_id, e
                        );
                    }
                }
            }
        }
        if updates > 0 {
            match convert_to_geo(g_boundary) {
                Some(g) => {
                    zone.boundary = Some(g);
                }
                None => {
                    println!(
                        "extrude_existing_town: failed to convert back to geo for {}...",
                        zone.osm_id
                    );
                    return false;
                }
            }
        }
    }
    true
}

fn get_parent_neighbors<'a, 'b>(
    parent: &Zone,
    towns: &'b [ZoneWithGeos<'a>],
    zones: &[Zone],
    zones_rtree: &ZonesTree,
    z_idx_to_place_idx: &HashMap<usize, usize>,
) -> Vec<&'b ZoneWithGeos<'a>> {
    zones_rtree
        .fetch_zone_bbox(&parent)
        .into_iter()
        .filter(|z_idx| is_city(&zones[z_idx.index]))
        .map(|z_idx| &towns[z_idx_to_place_idx[&z_idx.index]])
        .collect()
}

fn compute_voronoi<'a, 'b>(
    parent: &ZoneIndex,
    places: &[&Zone],
    zones: &[Zone],
    towns: &'b [ZoneWithGeos<'a>],
    zones_rtree: &ZonesTree,
    z_idx_to_place_idx: &HashMap<usize, usize>,
) -> Vec<Zone> {
    let points: Vec<(usize, Point<_>)> = places
        .iter()
        .enumerate()
        .filter_map(|(idx, p)| {
            if let Some(c) = p.center {
                Some((idx, c))
            } else {
                None
            }
        })
        .collect();
    let geos_points: Vec<(usize, Geometry<'_>)> = points
        .iter()
        .filter_map(|(pos, x)| {
            let x = match x.try_into() {
                Ok(x) => x,
                Err(e) => {
                    println!(
                        "Failed to convert point's center with id {}: {}",
                        places[*pos].osm_id, e
                    );
                    return None;
                }
            };
            Some((*pos, x))
        })
        .collect();
    let parent_index = parent.index;
    let parent = &zones[parent_index];
    let par = match match parent.boundary {
        Some(ref par) => geos::Geometry::try_from(par),
        None => {
            println!("No parent matches the index {}...", parent_index);
            return Vec::new();
        }
    } {
        Ok(par) => par,
        Err(e) => {
            println!(
                "Failed to convert parent with index {}: {}",
                parent.osm_id, e
            );
            return Vec::new();
        }
    };

    if points.len() == 1 {
        let mut place = places[0].clone();

        place.boundary = parent.boundary.clone();
        place.parent = Some(parent.id);
        if let Some(ref boundary) = place.boundary {
            place.bbox = boundary.bounding_rect();
        }
        let towns = get_parent_neighbors(&parent, towns, zones, zones_rtree, z_idx_to_place_idx);
        // If an error occurs, we can't just use the parent area so instead, we return nothing.
        if extrude_existing_town(&mut place, &towns) {
            return vec![place];
        }
        return Vec::new();
    }
    if parent.zone_type == Some(ZoneType::Country) {
        println!(
            "Parent {} is a country, ignoring all zones inside it:",
            parent.osm_id
        );
        for point in &points {
            println!(" => ignoring {}", places[point.0].osm_id);
        }
        return Vec::new();
    }
    let points_geom = match Geometry::create_geometry_collection(
        points
            .iter()
            .filter_map(|(_, p)| p.try_into().ok())
            .collect::<Vec<_>>(),
    ) {
        Ok(p) => p,
        Err(e) => {
            println!("Geometry::create_geometry_collection failed: {:?}", e);
            return Vec::new();
        }
    };
    let voronois = match points_geom.voronoi(Some(&par), 0., false) {
        Ok(v) => v,
        Err(e) => {
            println!(
                "Failed to compute voronoi for parent {}: {}",
                parent.osm_id, e
            );
            return Vec::new();
        }
    };
    let mut voronoi_polygons = Vec::with_capacity(points.len());
    let len = match voronois.get_num_geometries() {
        Ok(x) => x,
        Err(e) => {
            println!("get_num_geometries failed: {:?}", e);
            return Vec::new();
        }
    };
    for idx in 0..len {
        match voronois.get_geometry_n(idx) {
            Ok(x) => voronoi_polygons.push(x),
            Err(e) => {
                println!("get_geometry_n failed: {:?}", e);
            }
        }
    }

    // TODO: It "could" be better to instead compute the bbox for every new town and then call
    //       this function instead. To be checked...
    let towns = get_parent_neighbors(&parent, towns, zones, zones_rtree, z_idx_to_place_idx);
    voronoi_polygons
        .into_iter()
        .filter_map(|voronoi| {
            // Since GEOS doesn't return voronoi geometries in the same order as the given points...
            let mut place = {
                let x = geos_points
                    .iter()
                    .filter(|(_, x)| voronoi.contains(x).unwrap_or_else(|_| false))
                    .map(|(pos, _)| *pos)
                    .collect::<Vec<_>>();
                if !x.is_empty() {
                    places[x[0]].clone()
                } else {
                    println!("town not found for parent {}...", parent.osm_id);
                    return None;
                }
            };
            match voronoi.intersection(&par) {
                Ok(s) => {
                    place.parent = Some(parent.id);
                    place.boundary = convert_to_geo(s);
                    if let Some(ref boundary) = place.boundary {
                        place.bbox = boundary.bounding_rect();
                    }
                    extrude_existing_town(&mut place, &towns);
                    Some(place)
                }
                Err(e) => {
                    println!(
                        "intersection failure: {} ({})",
                        e,
                        voronoi
                            .get_context_handle()
                            .get_last_error()
                            .unwrap_or_else(|| "Unknown GEOS error".to_owned())
                    );
                    None
                }
            }
        })
        .collect()
}

fn publish_new_cities(zones: &mut Vec<Zone>, new_cities: Vec<Zone>) {
    for mut city in new_cities {
        city.id = ZoneIndex { index: zones.len() };
        zones.push(city);
    }
}
