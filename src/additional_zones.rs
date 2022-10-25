use crate::hierarchy_builder::ZonesTree;
use crate::is_place;
use crate::zone_ext::ZoneExt;
use anyhow::{Context, Result};
use cosmogony::{Zone, ZoneIndex, ZoneType};
use geo::prelude::BoundingRect;
use geo_types::{Coordinate, MultiPolygon, Point, Rect};
use geos::{Geom, Geometry};
use itertools::Itertools;
use osmpbfreader::{OsmId, OsmObj};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::collections::BTreeMap;

fn difference<'a>(g: &geos::Geometry<'a>, other: &Zone) -> Option<geos::Geometry<'a>> {
    let zone_as_geos: Option<Geometry> = other.boundary.as_ref().and_then(|b| {
        b.try_into()
            .map_err(|e| {
                warn!(
                    "Failed to convert boundary to geos Geometry for {}. Got {}",
                    other.osm_id, e
                );
            })
            .ok()
    });
    match zone_as_geos {
        Some(ref geom) => g
            .difference(geom)
            .map_err(|e| warn!("Geos difference failed for {}: {:?}", other.osm_id, e))
            .ok(),
        None => None,
    }
}

pub fn compute_additional_places(
    zones: &mut Vec<Zone>,
    parsed_pbf: &BTreeMap<OsmId, OsmObj>,
    zones_rtree: ZonesTree,
) {
    let place_zones = read_places(parsed_pbf);

    info!(
        "there are {} places, we'll try to make boundaries for them",
        place_zones.len()
    );

    let candidate_parent_zones = place_zones
        .par_iter()
        .filter_map(|place| {
            place.zone_type?;
            get_parent(place, zones, &zones_rtree).map(|parent| (parent, place))
        })
        .filter(|(parent, place)| {
            (parent.zone_type)
                .map(|parent_zone| {
                    if parent_zone == ZoneType::Country {
                        info!(
                            "Ignoring place with id {} and country {} as parent",
                            place.osm_id, parent.osm_id
                        );
                    }

                    // Ensuring zones are stricly increasing also ensures there will be no
                    // duplicates, for example by adding an admin label which is inside its
                    // boundary.
                    parent_zone > place.zone_type.unwrap_or(parent_zone)
                        && parent_zone < ZoneType::Country
                })
                .unwrap_or(false)
        })
        .fold(BTreeMap::<_, Vec<_>>::new, |mut map, (parent, place)| {
            map.entry(&parent.id).or_default().push(place);
            map
        })
        .reduce(BTreeMap::<_, Vec<_>>::new, |mut map1, map2| {
            for (k, mut v) in map2.into_iter() {
                map1.entry(k).or_default().append(&mut v);
            }
            map1
        });

    info!(
        "We'll compute voronois partitions for {} parent zones",
        candidate_parent_zones.len()
    );

    let new_cities: Vec<Zone> = {
        candidate_parent_zones
            .into_par_iter()
            .filter(|(_, places)| !places.is_empty())
            .map(|(parent, places)| compute_voronoi(parent, &places, zones, &zones_rtree))
            .flatten()
            .collect()
    };

    publish_new_places(zones, new_cities);
}

fn get_parent<'a>(place: &Zone, zones: &'a [Zone], zones_rtree: &ZonesTree) -> Option<&'a Zone> {
    zones_rtree
        .fetch_zone_bbox(place)
        .into_iter()
        .map(|z_idx| &zones[z_idx.index])
        .filter(|z| {
            // We would like to find a parent geometry used to build voronoi polygons
            // for all additional city points.
            // This parent geometry needs to represent a region whose type is larger than "City",
            // as it would not make sense to limit the extent of a city point
            // to the boundary of a city distinct (for instance).
            // Points which are already part of a "City" will be ignored afterwards.
            z.admin_type()
                .map(|zt| zt >= ZoneType::City)
                .unwrap_or(false)
        })
        .sorted_by_key(|z| z.zone_type)
        .find(|z| z.contains_center(place))
}

fn read_places(parsed_pbf: &BTreeMap<OsmId, OsmObj>) -> Vec<Zone> {
    parsed_pbf
        .values()
        .enumerate()
        .filter_map(|(index, obj)| {
            if !is_place(obj) {
                return None;
            }

            let node = obj.node()?;
            let next_index = ZoneIndex { index };
            let mut zone = Zone::from_osm_node(node, next_index)?;

            if zone.name.is_empty() {
                return None;
            }

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
            Some(zone)
        })
        .collect()
}

fn convert_to_geo(geom: Geometry<'_>) -> Result<MultiPolygon<f64>> {
    match geom.try_into().context("failed to convert to geo")? {
        geo::Geometry::Polygon(x) => Ok(MultiPolygon(vec![x])),
        geo::Geometry::GeometryCollection(geoms) => {
            // Convert each geometry into a multi-polygon
            let multi_polys: Vec<_> = geoms
                .into_iter()
                .map(MultiPolygon::try_from)
                .try_collect()?;

            // Flatten all multi polygons into a single one
            let polys = multi_polys
                .into_iter()
                .flat_map(|m| m.into_iter())
                .collect();

            Ok(MultiPolygon(polys))
        }
        y => Ok(y.try_into().context("failed to convert to multi-polygon")?),
    }
}

fn subtract_existing_zones(zone: &mut Zone, to_subtract: &[&Zone]) -> Result<()> {
    if to_subtract.is_empty() {
        return Ok(());
    }

    if let Some(ref boundary) = zone.boundary {
        let mut updates = 0;

        let mut g_boundary = geos::Geometry::try_from(boundary).map_err(|err| {
            warn!(
                "subtract_existing_town: failed to convert to geos for zone {}: {}",
                zone.osm_id, err
            );
            err
        })?;

        for z in to_subtract {
            if let Some(b) = difference(&g_boundary, z) {
                updates += 1;
                g_boundary = b;
            }
        }

        if updates > 0 {
            let g = convert_to_geo(g_boundary).map_err(|err| {
                warn!(
                    "subtract_existing_town: failed to convert back to geo for {}...",
                    zone.osm_id
                );
                err
            })?;

            zone.bbox = g.bounding_rect();
            zone.boundary = Some(g);
        }
    }
    Ok(())
}

fn get_places_to_subtract<'a>(
    zone: &Zone,
    parent_id: &ZoneIndex,
    zones: &'a [Zone],
    zones_rtree: &ZonesTree,
) -> Vec<&'a Zone> {
    zones_rtree
        .fetch_zone_bbox(zone)
        .into_par_iter()
        .map(|z_idx| &zones[z_idx.index])
        .filter(|z| {
            z.admin_type()
                .map(|zt| zt <= ZoneType::City || z.parent == Some(*parent_id))
                .unwrap_or(false)
        })
        .filter(|z| zone.intersects(z))
        .collect()
}

fn compute_voronoi(
    parent: &ZoneIndex,
    places: &[&Zone],
    zones: &[Zone],
    zones_rtree: &ZonesTree,
) -> Vec<Zone> {
    let points: Vec<(usize, Point<_>)> = places
        .iter()
        .enumerate()
        .filter_map(|(idx, p)| p.center.map(|c| (idx, c)))
        .collect();

    let parent_index = parent.index;
    let parent = &zones[parent_index];

    if points.len() == 1 {
        let mut place = places[0].clone();
        place.boundary = parent.boundary.clone();
        place.bbox = parent.bbox;
        place.parent = Some(parent.id);
        let zones_to_subtract = get_places_to_subtract(parent, &parent.id, zones, zones_rtree);
        // If an error occurs, we can't just use the parent area so instead, we return nothing.
        if subtract_existing_zones(&mut place, &zones_to_subtract).is_ok() {
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
            warn!("Geometry::create_geometry_collection failed: {:?}", e);
            return Vec::new();
        }
    };

    let geos_parent = match match parent.boundary {
        Some(ref par) => geos::Geometry::try_from(par),
        None => {
            warn!("Parent {} has no boundary", parent.osm_id);
            return Vec::new();
        }
    } {
        Ok(par) => par,
        Err(e) => {
            warn!("Failed to convert parent {} to geos: {}", parent.osm_id, e);
            return Vec::new();
        }
    };

    let voronois = match points_geom.voronoi(Some(&geos_parent), 1e-5, false) {
        Ok(v) => v,
        Err(e) => {
            warn!(
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
            warn!("get_num_geometries failed: {:?}", e);
            return Vec::new();
        }
    };
    for idx in 0..len {
        match voronois.get_geometry_n(idx) {
            Ok(x) => voronoi_polygons.push(x),
            Err(e) => {
                warn!("get_geometry_n failed: {:?}", e);
            }
        }
    }

    let geos_points: Vec<(usize, Geometry<'_>)> = points
        .iter()
        .filter_map(|(pos, x)| {
            let x = match x.try_into() {
                Ok(x) => x,
                Err(e) => {
                    warn!(
                        "Failed to convert point's center with id {}: {}",
                        places[*pos].osm_id, e
                    );
                    return None;
                }
            };
            Some((*pos, x))
        })
        .collect();

    voronoi_polygons
        .into_par_iter()
        .filter_map(|voronoi| {
            // WARNING: This clone should not be necessary, but segfaults occured. Thread-safety issue in geos ?
            let geos_points = geos_points.clone();

            // Since GEOS doesn't return voronoi geometries in the same order as the given points...
            let mut place = {
                if let Some(idx) = geos_points
                    .iter()
                    .filter(|(_, x)| voronoi.contains(x).unwrap_or(false))
                    .map(|(pos, _)| *pos)
                    .next()
                {
                    places[idx].clone()
                } else {
                    println!("town not found for parent {}...", parent.osm_id);
                    return None;
                }
            };

            match geos_parent.intersection(&voronoi) {
                Ok(s) => {
                    place.parent = Some(parent.id);

                    place.boundary = convert_to_geo(s)
                        .map_err(|err| warn!("failed to convert to geos: {err:?}"))
                        .ok();

                    if let Some(ref boundary) = place.boundary {
                        place.bbox = boundary.bounding_rect();
                    }
                    let zones_to_subtract =
                        get_places_to_subtract(&place, &parent.id, zones, zones_rtree);
                    subtract_existing_zones(&mut place, &zones_to_subtract).ok()?;
                    Some(place)
                }
                Err(e) => {
                    warn!(
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

fn publish_new_places(zones: &mut Vec<Zone>, new_cities: Vec<Zone>) {
    for mut city in new_cities {
        city.id = ZoneIndex { index: zones.len() };
        zones.push(city);
    }
}
