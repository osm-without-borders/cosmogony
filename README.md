# cosmogony

:construction::warning: This is a work in progress, for the moment we have ideas but not code :warning::construction:

##  Goals
The goal of the project is to have easy to use, easy to update geographic regions.

[OpenStreetMap](https://www.openstreetmap.org) (OSM) seems the best datasource for this, but the OSM administrative regions (admins) have several drawbacks.

 * admin_level : The world is a complicated place, and each country has its own administrative division. OSM uses an `admin_level`tag, with values from 1 to ~10 to allow consistent rendering of the borders among countries. This is fine for making maps, but if you want a world list of cities or regions, you still need local and specific knowledge to find which admin_level to use in each country.
 * no hierarchy (:construction:)

We need a structured admin hierarchy to easily known that [paris](https://www.openstreetmap.org/relation/7444) is `city` in the `state` [ile de france](https://www.openstreetmap.org/relation/8649) in the `country` [france](https://www.openstreetmap.org/relation/2202162).

OSM administrative regions may not be mapped with the same precision all over the earth but the data is easy to update and the update will benefit the community.

We do not forbid ourself however to use other data sources (with compliant license), but we don't want `cosmogony` to be too complex and we do not aim to recreate the great [WhosOnFirst](https://www.whosonfirst.org/) ([see below](#See also))

## Use cases

### Our use case
We need this in our geocoder, [mimir](https://github.com/CanalTP/mimirsbrunn) where we need an extended knowledge of the administrative regions.

See [the founding issue](https://github.com/CanalTP/mimirsbrunn/issues/178) for a bit of context.

### Others
:construction:

## Features

### General idea
The initial idea of the project is to take an OSM pbf file and:
 * use geometric algorithm to define which admin belong to another admin (we'll start with shapes exact inclusion and see if that's enough)
 * use the [well defined libpostal rules](https://github.com/openvenues/libpostal/tree/master/resources/boundaries/osm) to type the admin depending on it's country

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

## Future
:construction:

## Dataset quality test
:construction: how we plan to ensure the quality of the released dataset :construction:

## See also

## Licenses
All code in this repository is under the [Apache License 2.0](./LICENSE).

This project uses OpenStreetMap data, licensed under the ODbL by the OpenStreetMap Foundation. * You need to visibly credit OpenStreetMap and its contributors if you use or distribute the data from cosmogony.
Read more on [OpenStreetMap official website](https://www.openstreetmap.org/copyright).
