# cosmogony data for tests

## luxembourg_filtered.osm.pbf

An OSM PBF from 2018 with Luxembourg country, filtered on boundaries elements.

Expected relations:

* `admin_level = 2`: 1 (and 3 incomplete)
* `admin_level = 3`: none (and 1 incomplete)
* `admin_level = 4`: none (and a few incomplete ones)
* `admin_level = 5`: none
* `admin_level = 6`: 13 (and a few incomplete ones)
* `admin_level = 7`: none (and a few incomplete ones)
* `admin_level = 8`: 104 (and a few incomplete ones. One of them only have the admin center missing out)
* `admin_level = 9`: 79 (and a few incomplete ones)
* `admin_level = 10`: 2 (and many incomplete ones)

## gatineau pbf

This PBF contains a single relation that has no "admin_center" role node but a "label" one.
