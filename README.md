# cosmogony

:construction::warning: This is a work in progress, for the moment we have ideas but not code :stuck_out_tongue_winking_eye: :warning::construction:

##  Goals
The goal of the project is to simplify the uses of administrative regions (admins) in [OpenStreetMap](https://www.openstreetmap.org).

The OSM admins have several problems:
 * admin_level (:construction:)
 * no hierarchy (:construction:)

We need a structured admin hierarchy to easily known that [paris](https://www.openstreetmap.org/relation/7444) is `city` in the `state` [ile de france](https://www.openstreetmap.org/relation/8649) in the `country` [france](https://www.openstreetmap.org/relation/2202162).

## Use cases

### Our use case
We need this in our geocoder, [mimir](https://github.com/CanalTP/mimirsbrunn) we need an extended knowledge of the administrative regions.
see [the founding issue](https://github.com/CanalTP/mimirsbrunn/issues/178) for a bit of context.

### Others
:construction:

## Features

### General idea
The initial idea of the project is to take an OSM pbf file and:
 * use geometric algorithm to define which admin belong to another admin (we'll start with shapes exacte inclusion and see if that's enough)
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

## See also

## Licences
:construction:
### Code
:construction:
### Data
:construction:
