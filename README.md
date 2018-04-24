# cosmogony

:construction::warning: This is a work in progress. Take a look at the issues if you want to contribute :warning::construction:

The goal of the project is to have easy to use, easy to update geographic regions.

The data can be browsed at http://cosmogony.world.

It provides geographical zones with a structured hierarchy to easily know that [Paris](https://www.openstreetmap.org/relation/7444) is `city` in the `state` [Île-de-France](https://www.openstreetmap.org/relation/8649) in the `country` [France](https://www.openstreetmap.org/relation/2202162).

The general idea of the project is to take OpenStreetMap data and:
 * use the [well defined libpostal rules](https://github.com/openvenues/libpostal/tree/master/resources/boundaries/osm) to type each zone according to its country
 * use geographic inclusion to build a hierarchy

## Use

### Get data
:construction: We may provide direct data download in the future. For now, you have to extract your geographic regions by yourself :construction:

### Create data
You can build cosmogony to extract the regions on your own.
#### Build
You will need
* rust (`curl https://sh.rustup.rs -sSf | sh`)
* GEOS (`apt-get install libgeos-dev`)

Clone this repo and update the git submodules (`git submodule update --init`)

Then, build cosmogony: `cargo build --release`

#### Run

You can now grab some OSM pbf and extract your geographic zones:

`cargo run --release -- -i /path/to/your/file.osm.pbf`

Check out cosmogony help for more options: `cargo run --release -- -h`

### Use data

You can get an idea of the coverage, view zones metadata and inspect the hierarchy with our awesome [Cosmogony Explorer]( https://github.com/osm-without-borders/cosmogony_explorer)

:construction: In the future, we may provide other tools to explore, debug and use the data. Please share your ideas and needs in the issues :construction:

## Why ?

### Our use case
We need this in our geocoder, [mimir](https://github.com/CanalTP/mimirsbrunn) where we need an extended knowledge of the administrative regions.

See [the founding issue](https://github.com/CanalTP/mimirsbrunn/issues/178) for a bit of context.

### Others
:construction:

## Features

### Data sources and algorithm

[OpenStreetMap](https://www.openstreetmap.org) (OSM) seems the best datasource for our use case, but the OSM administrative regions (admins) have several drawbacks.

 * admin_level : The world is a complicated place, and each country has its own administrative division. OSM uses an `admin_level` tag, with values from 1 to ~10 to allow consistent rendering of the borders among countries. This is fine for making maps, but if you want a world list of cities or regions, you still need local and specific knowledge to find which admin_level to use in each country.
 * no hierarchy


 To mitigate this, the general idea is to take an OSM pbf file and:
  * use geometric algorithm to define which admin belong to another admin (we'll start with shapes exact inclusion and see if that's enough)
  * use the [libpostal rules](https://github.com/openvenues/libpostal/tree/master/resources/boundaries/osm) to type the admin depending on its country


OSM administrative regions may not be mapped with the same precision all over the earth but the data is easy to update and the update will benefit the community.

We do not forbid ourself however to use other data sources (with compliant license), but we don't want `cosmogony` to be too complex and we do not aim to recreate the great [WhosOnFirst](https://www.whosonfirst.org/) ([see below](#See-also))

### Administrative types
The libpostal types seems nice (and made by brighter people than us):

- **suburb**: usually an unofficial neighborhood name like "Harlem", "South Bronx", or "Crown Heights"
- **city_district**: these are usually boroughs or districts within a city that serve some official purpose e.g. "Brooklyn" or "Hackney" or "Bratislava IV"
- **city**: any human settlement including cities, towns, villages, hamlets, localities, etc.
- **state_district**: usually a second-level administrative division or county.
- **state**: a first-level administrative division. Scotland, Northern Ireland, Wales, and England in the UK are mapped to "state" as well (convention used in OSM, GeoPlanet, etc.)
- **country_region**: informal subdivision of a country without any political status
- **country**: sovereign nations and their dependent territories, anything with an [ISO-3166 code](https://en.wikipedia.org/wiki/ISO_3166-1_alpha-2).

### Output schema
:construction:

## Dataset quality test
:construction: how we plan to ensure the quality of the released dataset. Contributions welcomed in [issue #4](https://github.com/osm-without-borders/cosmogony/issues/4) :construction:

## See also
#### [Mapzen borders](https://mapzen.com/data/borders/) project
deprecated, and without cascading hierarchy

#### [WhosOnFirst](https://www.whosonfirst.org/)
Our main inspiration source :sparkling_heart:
Hard to maintain because of the many sources involved that needs deduplication and concordances, difficult to ensure a coherent hierarchy (an object Foo can have an object Bar as a child whereas Foo is not listed as a parent of Bar), etc

#### [OSM boundaries Map](https://wambachers-osm.website/boundaries/)
Pretty cool if you just need to inspect the coverage or export a few administrative areas. Still need country specific knowledge to use worldwide.

#### WhateverShapes : [quattroshapes](https://github.com/foursquare/quattroshapes), alphashapes, [betashapes](https://github.com/simplegeo/betashapes)
Without cascading hierarchy. Duno if it's up to date, and how we can contribute.


## Licenses
All code in this repository is under the [Apache License 2.0](./LICENSE).

This project uses OpenStreetMap data, licensed under the ODbL by the OpenStreetMap Foundation. You need to visibly credit OpenStreetMap and its contributors if you use or distribute the data from cosmogony.
Read more on [OpenStreetMap official website](https://www.openstreetmap.org/copyright).
