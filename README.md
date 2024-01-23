# Cosmogony
[![Github workflow](https://github.com/osm-without-borders/cosmogony/actions/workflows/test-and-publish.yml/badge.svg?branch=master)](https://github.com/osm-without-borders/cosmogony/actions)
[![Crates.io](https://img.shields.io/crates/v/cosmogony.svg)](https://crates.io/crates/cosmogony)
 [![Crates.io](https://img.shields.io/crates/d/cosmogony.svg)](https://crates.io/crates/cosmogony)

This is home to Cosmogony, a project that aims at providing an efficient tool to quickly use and update worldwide geographical regions. It returns geographical zones with a structured hierarchy to easily know that [Paris](https://www.openstreetmap.org/relation/7444) is a `city` in the `state` [Île-de-France](https://www.openstreetmap.org/relation/8649) in the `country` [France](https://www.openstreetmap.org/relation/2202162). The architecture of Cosmogony is based on [OpenStreetMap data](https://www.openstreetmap.org) and on the exploitation of [well defined libpostal rules](https://github.com/openvenues/libpostal/tree/master/resources/boundaries/osm) to type each zone according to its country. Then the resulting hierarchy is built thanks to geographical inclusion. An example of a full data extract can be browsed at http://cosmogony.world.

To explore and navigate fluently in the built hierarchy, Cosmogony comes along with two other tools:
- :link: [Cosmogony Explorer](https://github.com/osm-without-borders/cosmogony_explorer)
- :link: [Cosmogony Dashboard](https://github.com/osm-without-borders/cosmogony-data-dashboard)

Below is a brief visualisation of a basic use case of the Cosmogony Explorer:

<img src="https://github.com/osm-without-borders/cosmogony_explorer/raw/master/demo.gif" width="750" title="Explore the Cosmogony in our explorer">

## Getting started

### Get data

:construction: Until we propose a direct data download, you have to extract your geographic regions by yourself (see below). :construction:

### Use data

The best way to explore the data (i.e. the coverage, the zones metadata, the hierarchy...) is our [Cosmogony Explorer](https://github.com/osm-without-borders/cosmogony_explorer)

:construction: In the future, we may create other tools to use the data. Please share your ideas and needs in the issues. :construction:

### Extract data

You can build cosmogony to extract the regions on your own.

- #### Build
Here are the necessary manual steps to build cosmogony :
```bash
curl https://sh.rustup.rs -sSf | sh    # intall rust
apt-get install libgeos-dev            # install GEOS
git clone https://github.com/osm-without-borders/cosmogony.git     # Clone this repo
cd cosmogony;                          # enter the directory
git submodule update --init            # update the git submodules
cargo build --release                  # finally build cosmogony
```

- #### Run

You can now grab some OSM pbf and extract your geographic zones:
`cargo run --release -- generate -i /path/to/your/file.osm.pbf`

Check out cosmogony help for more options:
`cargo run --release -- -h`

- #### Other subcomands

Note: the default subcommand is the `generate` subcommand, so `cosmogony -i <osm-file> -o output file` if the same as `cosmogony generate -i <osm-file> -o output file`

- ##### Merging cosmogonies

To generate a world cosmogony on a server withtout a lot of RAM, you can generate cosmogonies on split non overlapping osm files, without a shared parent (eg. split by continent or country) and merge the generated cosmogony.

To merge several cosmogonies into one you can use the custom subcommand `merge`:
`cargo run --release -- merge *.jsonl -o merged_cosmo.jsonl`

Note: to reduce the memory footprint, it can only merge json lines  cosmogonies (so `.jsonl` or `.jsonl.gz`). 

## Documentation

The initial purpose of Cosmogony is to enhance [mimir](https://github.com/CanalTP/mimirsbrunn), our geocoder (See [the founding issue](https://github.com/CanalTP/mimirsbrunn/issues/178) for a bit of context).
Another common use case is to create geospatially aware statistics, such as choropleth maps.
Anyway, we'd love to know what you've built from this, so feel free to add your use cases in [Awesome Cosmogony](awesome.md).

### Data sources and algorithm

[OpenStreetMap](https://www.openstreetmap.org) (OSM) seems the best datasource for our use case. However the OSM administrative regions (admins) have several drawbacks:
 * **admin_level**: The world is a complex place where each country has its own administrative division. OSM uses an `admin_level` tag, with values ranging from 1 to ~10 to allow consistent rendering of the borders among countries. This is fine for making maps, but if you want a world list of cities or regions, you still need local and specific knowledge to find which admin_level to use in each country.
 * **no existing hierarchy**: indeed the OSM data model rests only on `nodes`, `ways` and `relation` without any structure.

 To mitigate this, the general idea is to take an OSM pbf file and to:
* use a geometric algorithm to define which admin belongs to another admin (we'll start with shapes exact inclusion and see if that's enough).
* use the [libpostal rules](https://github.com/openvenues/libpostal/tree/master/resources/boundaries/osm) to type the admin depending on its country.

OSM administrative regions may not be mapped with the same precision all over the earth but the data is easy to update and the update will benefit the community.

Beyond OSM, we will possibly consider in the future using other data sources (with compliant license).
However we don't want `cosmogony` to be too complex (as the great [WhosOnFirst](https://www.whosonfirst.org/) is ([see below](#See-also))

### Administrative types

The libpostal types seem nice (and made by brighter people than us):

- **suburb**: usually an unofficial neighborhood name like "Harlem", "South Bronx", or "Crown Heights"
- **city_district**: these are usually boroughs or districts within a city that serve some official purpose e.g. "Brooklyn" or "Hackney" or "Bratislava IV"
- **city**: any human settlement including cities, towns, villages, hamlets, localities, etc.
- **state_district**: usually a second-level administrative division or county.
- **state**: a first-level administrative division. Scotland, Northern Ireland, Wales, and England in the UK are mapped to "state" as well (convention used in OSM, GeoPlanet, etc.)
- **country_region**: informal subdivision of a country without any political status
- **country**: sovereign nations and their dependent territories, anything with an [ISO-3166 code](https://en.wikipedia.org/wiki/ISO_3166-1_alpha-2).

### Names and Labels

Cosmogony reads OSM tags to determine names and labels for all zones, in all available languages.

In addition to `name:*` tags from boundary objects themselves, other names from [related objects](https://wiki.openstreetmap.org/wiki/Relation:boundary#Relation_members) are used
as they may provide more languages : 
 * nodes with role `label` (if present)
 * nodes with role `admin_center` (if relevant: for cities, or on matching wikidata ID)

> Note that these additional `name:*` values **are included in zone `tags`** in the output to help reusing, even if they are not part of the OSM object tags.

### Output schema

Below is a brief example of the information contained in the cosmogony output.

```javascript
{
	"zones":[
		{"id":0,
		"osm_id":"relation:110114",
		"admin_level":8,
		"zone_type":"city",
		"name":"Sand Rock",
		"zip_codes":[],
		"center":{"coordinates":[-85.77153961457083,34.2303942501858],"type":"Point"},
		"bbox": [-85.803571, 34.203915, -85.745058, 34.26666],
		"geometry":{
			"coordinates":"..."
		},
		"tags":{
			"admin_level":"8",
			"border_type":"city",
			"boundary":"administrative",
			"is_in":"USA"
		},
		"parent":"null",
		"wikidata":"Q79669"}
	],
		"meta":{
			"osm_filename":"alabama.osm.pbf",
			"stats":{"level_counts":{"6":64,"8":272},
			"zone_type_counts":{"City":272,"StateDistrict":64},
			"wikidata_counts":{"6":58,"8":202},
			"zone_with_unkwown_country_rules":{},
			"unhandled_admin_level":{},
			"zone_without_country":0}
		}
}
```

## Dataset quality test

You can check the cosmogony file built with our [Cosmogony Data Dashboard](https://github.com/osm-without-borders/cosmogony-data-dashboard).

:construction: Ideas and other contributions welcomed in [issue #4](https://github.com/osm-without-borders/cosmogony/issues/4) :construction:

## Contribute

Cosmogony, just like OpenStreetMap, emphasizes local knowledge: even if you can't code, you can help us to make Cosmogony go worldwide :rocket:

If the cosmogony of your country does not look good, here is what you can do to fix it:

### Tell us which administrative zones are relevant and how to extract them from OSM

* Find your country here: https://github.com/osm-without-borders/libpostal/tree/master/resources/boundaries/osm
* Edit the config file to map the relevant administrative zones with libpostal types and OSM admin_level
    * the [OSM wiki page](https://wiki.openstreetmap.org/wiki/Tag:boundary%3Dadministrative#10_admin_level_values_for_specific_countries) about admin_level may be useful
    * [The French config file](https://github.com/osm-without-borders/libpostal/blob/master/resources/boundaries/osm/fr.yaml) is a good example if you need inspiration
* [Make a Pull Request](http://makeapullrequest.com/) with your changes

### Tell us how many administrative zones are expected

* Find a reliable data source (Wikipedia, [Wikidata](https://github.com/osm-without-borders/cosmogony-data-dashboard/blob/master/wikidata.md), [Eurostat NUTS & LAU](https://ec.europa.eu/eurostat/web/nuts/local-administrative-units), etc)
* Update the reference file from our [Data Dashboard](http://cosmogony.world/#/data_dashboard) with the number of zones that actually exists in the country: https://github.com/osm-without-borders/cosmogony-data-dashboard/blob/master/reference_stats_values.csv
    * If you are unsure if the right number of cities is 3314 or 3322, you can use the `expected_min` and `expected_max` columns :wink:
    * If the number of zones already in OSM does not match the expected number, please mark the test by putting `yes` in the `is_known_failure` column
* [Make a Pull Request](http://makeapullrequest.com/) with your changes


## See also

- #### [Mapzen borders](https://mapzen.com/data/borders/) project

deprecated, and without cascading hierarchy

- #### [WhosOnFirst](https://www.whosonfirst.org/)

Our main inspiration source :sparkling_heart:
Hard to maintain because of the many sources involved that needs deduplication and concordances, difficult to ensure a coherent hierarchy (an object Foo can have an object Bar as a child whereas Foo is not listed as a parent of Bar), etc

- #### [OSM boundaries Map](https://wambachers-osm.website/boundaries/)

Pretty cool if you just need to inspect the coverage or export a few administrative areas. Still need country specific knowledge to use worldwide.

- #### WhateverShapes : [quattroshapes](https://github.com/foursquare/quattroshapes), alphashapes, [betashapes](https://github.com/simplegeo/betashapes)

Without cascading hierarchy. Duno if it's up to date, and how we can contribute.

## Licenses

All code in this repository is under the [Apache License 2.0](./LICENSE).

This project uses OpenStreetMap data, licensed under the ODbL by the OpenStreetMap Foundation. You need to visibly credit OpenStreetMap and its contributors if you use or distribute the data from cosmogony.
Read more on [OpenStreetMap official website](https://www.openstreetmap.org/copyright).

