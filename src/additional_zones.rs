use osmpbfreader::{OsmId, OsmObj, OsmPbfReader};
use std::collections::BTreeMap;
use std::fs::File;
use zone::{Zone, ZoneIndex, ZoneType};
use zone_tree::ZonesTree;
use geos;
use geo_types::Point;

pub fn compute_additional_cities(zones: &mut Vec<Zone>, pbf_reader: &mut OsmPbfReader<File>) {
    let place_zones = read_places(pbf_reader);
    info!("there are {} places, we'll try to make boundaries for them",place_zones.len());
    let zones_rtree: ZonesTree = zones.iter().filter(|z| z.is_admin()).collect();

    let new_cities: Vec<Zone> = {
        let mut candidate_parent_zones: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (parent, place) in place_zones
            .into_iter()
            .filter_map(|place| get_parent(&place, &zones, &zones_rtree).map(|parent| (parent, place)))
            .filter(|(parent, _)| parent.zone_type == Some(ZoneType::City))
        {
            candidate_parent_zones
                .entry(&parent.id)
                .or_default()
                .push(place);
        }

        info!("We'll compute voronois partitions for {} parent zones", candidate_parent_zones.len());

        candidate_parent_zones.into_iter() //TODO into_par_iter
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
        .filter(|z| z.contains(place))
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

fn read_places(pbf_reader: &mut OsmPbfReader<File>) -> Vec<Zone> {
    let mut zones = vec![];

    for obj in pbf_reader.par_iter().map(Result::unwrap) {
        if !is_place(&obj) {
            continue;
        }
        if let OsmObj::Node(ref node) = obj {
            let osm_id = OsmId::Node(node.id);
            let next_index = ZoneIndex { index: zones.len() };
            if let Some(mut zone) = Zone::from_osm(&node.tags, next_index, osm_id) {
                zone.zone_type = Some(ZoneType::City);
                zone.center = Some(Point::<f64>::new(node.lon(), node.lat()));
                zones.push(zone);
            }
        }
    }
    zones
}

fn compute_voronoi(parent: &ZoneIndex, places: Vec<Zone>, zones: &[Zone]) -> Vec<Zone> {

    // let points = places.iter().map(|p| p)
    unimplemented!()
}

fn publish_new_cities(zones: &mut Vec<Zone>, new_cities: Vec<Zone>) {
    for mut city in new_cities {
        city.id = ZoneIndex { index: zones.len() };
        zones.push(city);
    }
}