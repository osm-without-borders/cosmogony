use failure::ResultExt;
use geo_types::{Coordinate, MultiPolygon, Point, Polygon, Rect};
use osmpbfreader::{OsmObj, OsmPbfReader};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use crate::zone::{Zone, ZoneIndex, ZoneType};
use crate::zone_tree::ZonesTree;
use geos::from_geo::TryInto;
use geos::GGeom;

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
            .filter_map(|mut place| {
                let original = place.clone();
                println!("==> looking for {:?}", place.id);
                while let Some(par) = get_parent(&place, &zones, &zones_rtree) {
                    if par.id == original.id || place.id == par.id {
                        break
                    }
                    if (par.zone_type != Some(ZoneType::City) &&
                        par.zone_type != Some(ZoneType::Suburb)) ||
                       par.boundary.is_none() {
                        place = &par;
                        continue
                    }
                    info!(
                        "on a trouve le parent {:?} / {:?} for {:?}",
                        &par.id, &par.zone_type, original.id
                    );
                    return Some((par, original))
                }
                info!("no parent, let's continue...");
                let mut potential: Option<(Zone, usize)> = None;
                // Didn't find a parent so let's do in another way!
                for (pos, zone) in zones.iter().enumerate() {
                    if zone.id == original.id || zone.zone_type == Some(ZoneType::City) ||
                       zone.zone_type == Some(ZoneType::Suburb) {
                        continue;
                    }
                    if let (Some(ref boundary), Some(ref sub_boundary)) = (&zone.boundary, &original.boundary) {
                        let boundary = boundary.try_into().expect("failed to convert to geos");
                        let sub_boundary = sub_boundary.try_into().expect("failed to convert to geos");
                        if boundary.contains(&sub_boundary).expect("failed to use contains") {
                            if match potential {
                                Some((ref p, _)) => {
                                    if let Some(ref bound) = p.boundary {
                                        bound.try_into().unwrap().area().unwrap() < boundary.area().unwrap()
                                    } else {
                                        false
                                    }
                                }
                                None => {
                                    true
                                }
                            } {
                                potential = Some((zone.clone(), pos));
                            }
                        }
                    }
                }
                if let Some((_, pos)) = potential {
                    println!("finally found one!");
                    Some((&zones[pos], original))
                } else {
                    println!("still no parent found...");
                    None
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
            .map(|(parent, mut places)| compute_voronoi(parent, &mut places, &zones))
            .flatten()
            // .map_err(|e| error!("error while computing voronoi : {}", e))
            .collect()
    };
    /*if !new_cities.iter().any(|x| x.zone_type == Some(ZoneType::Country)) {
        if let Some(x) = place_zones.iter().find(|x| x.zone_type == Some(ZoneType::Country)) {
            new_cities.push(x.clone());
        } else {
            println!("Couldn't find a country...");
        }
    }*/

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

fn compute_voronoi(parent: &ZoneIndex, places: &[Zone], zones: &[Zone]) -> Vec<Zone> {
    //TODO remove this collect by changing the compute_voronoi function signature
    let points: Vec<Point<_>> = places.iter().filter_map(|p| p.center).collect();

    let voronois = geos::compute_voronoi(&points, 0.).unwrap(); //TODO remove unwrap

    let calc_geom = create_forbidden_geom(&zones[parent.index], zones);
    voronois.into_iter().enumerate().map(|(idx, voronoi)| {
        let mut place: Zone = places[idx].clone();

        use geo::prelude::Area;
        let before_area = voronoi.area();
        place.boundary = make_boundary(voronoi, &calc_geom);
        if calc_geom.is_some() {
            if let Some(ref b) = place.boundary {
                println!("Change of size: {} => {} {}", before_area, b.area(), place.name);
            }
        }
        //TODO compute the bounding box
        place.parent = Some(parent.clone());
        place
    }).filter(|z| z.boundary.is_some())
      .collect() // TODO remove this collect

    //TODO if possible zip it
    // for (ref mut place, voronoi) in places.iter_mut().filter(|p| p.center.is_some()).zip(voronois.into_iter()){
    // };
}

/// Create a multipolygon with the boundary of the parent zone + all the cities with real boundaries from this zone
/// This will be use to extract this geometry from the
fn create_forbidden_geom(parent_zone: &Zone, places: &[Zone]) -> Option<GGeom> {
    if let Some(ref parent) = parent_zone.boundary {
        println!("HELLLLLLOOOOOOO {:?} {}", parent_zone.zone_type, parent_zone.name);
        let parent: GGeom = parent.try_into()
                                  .expect("cannot convert to multipolygon");
        let places = places.iter()
                           .filter(|x| x.zone_type == Some(ZoneType::City)/* || x.zone_type == Some(ZoneType::Suburb)*/)
                           .filter_map(|x| x.boundary.as_ref())
                           .filter_map(|x| x.try_into().ok())
                           .filter(|x| !parent.contains(x).expect("failed contains"));
                           //.filter(|x| { let x = x.intersects(&parent).expect("intersects FAILED... badly"); println!("||||> {:?}", x); x});
        let mut x = 0;
        let mut geom: Option<GGeom> = None;
        for place in places {
            x += 1;
            geom = Some(match geom {
                Some(g) => {
                    if x == 1 {
                        println!("union BETWEEN \"{}\"\nand \"{}\"", g.to_wkt(), place.to_wkt());
                    }
                    g.union(&place).expect("union failed")
                },
                None => place.clone(),
            });
        }
        println!("µµµµµµµµ>> LOOPED OVER {} elems", x);
        geom.map(|g| g.union(&parent).expect("failed union").intersection(&parent).expect("intersection failed"))
    } else {
        None
    }
}

fn make_boundary(
    voronoi: Polygon<f64>,
    geom_to_extract: &Option<GGeom>,
) -> Option<MultiPolygon<f64>> {
    if let Some(forbidden) = geom_to_extract {
        let voronoi: GGeom = voronoi.try_into().expect("invalid voronoi");

        //println!("====> YEAY!! {}\n||||||> {}", forbidden.to_wkt(), voronoi.to_wkt());
        if let Some(x) = voronoi.intersection(forbidden).ok().map(|x| {
            /*MultiPolygon(vec![x.try_into()
                               .expect("failed to convert")
                               .into_polygon()
                               .expect("not a polygon")])*/
            match x.try_into().expect("t") {
                geo::Geometry::Polygon(x) => Some(MultiPolygon(vec![x])),
                y => {
                    println!("=> {:?}", y);
                    if let Some(x) = y.into_multi_polygon() {
                        Some(x)
                    } else {
                        println!("!!! not a multipolygon...");
                        None
                    }
                }
            }
        }) {
            x
        } else {
            None
        }
    } else {
        println!("No forbidden zone, going for \"normal\"");
        Some(MultiPolygon(vec![voronoi]))
    }
}

fn publish_new_cities(zones: &mut Vec<Zone>, new_cities: Vec<Zone>) {
    for mut city in new_cities {
        city.id = ZoneIndex { index: zones.len() };
        zones.push(city);
    }
}
