use cosmogony::{Zone, ZoneIndex, ZoneType};
/* use failure::Fail; */
/* use failure::{err_msg, Error}; */
use anyhow::{anyhow, Error};
use log::warn;
use serde_derive::*;
use std::collections::BTreeMap;
use std::fmt;

use include_dir::{include_dir, Dir};

// :warning:
// The include_dir macro cannot cannot watch for changes to rebuild,
// so if you only change the libpostal rules, you need either to clean the repo before building
// or just touch this file to trigger a reimport
const LIBPOSTAL_RULES_DIR: Dir = include_dir!("./libpostal/resources/boundaries/osm/");

#[derive(Debug)]
pub struct ZoneTyper {
    countries_rules: BTreeMap<String, CountryAdminTypeRules>,
}

#[derive(Deserialize, Ord, PartialOrd, Eq, PartialEq, Debug)]
enum OsmPrimaryObjects {
    #[serde(rename = "node")]
    Node,
    #[serde(rename = "way")]
    Way,
    #[serde(rename = "relation")]
    Relation,
}

impl fmt::Display for OsmPrimaryObjects {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OsmPrimaryObjects::Node => fmt.write_str("node")?,
            OsmPrimaryObjects::Way => fmt.write_str("way")?,
            OsmPrimaryObjects::Relation => fmt.write_str("relation")?,
        }
        Ok(())
    }
}

#[derive(Default, Debug)]
struct RulesOverrides {
    contained_by: BTreeMap<String, CountryAdminTypeRules>,
    id_rules: BTreeMap<String, Option<ZoneType>>,
}

#[derive(Deserialize, Debug)]
struct CountryAdminTypeRules {
    #[serde(rename = "admin_level", default)]
    type_by_level: BTreeMap<String, ZoneType>,
    #[serde(
        default,
        deserialize_with = "de_with_from::<_, SerdeRulesOverrides, _>"
    )]
    overrides: RulesOverrides,
    // we don't implement libpostal's 'use_admin_center' as we don't need it
}

pub enum ZoneTyperError {
    InvalidCountry(String),
    UnkownLevel(Option<u32>, String),
}

impl ZoneTyper {
    pub fn new() -> Result<ZoneTyper, Error> {
        let z = ZoneTyper {
            countries_rules: read_libpostal_yaml_folder()?,
        };
        if z.countries_rules.is_empty() {
            Err(anyhow!(
                "no country rules have been loaded, the libpostal directory \
                 must contains valid libpostal rules"
            ))
        } else {
            Ok(z)
        }
    }

    pub fn get_zone_type(
        &self,
        zone: &Zone,
        country_code: &str,
        zone_inclusions: &[ZoneIndex],
        all_zones: &[Zone],
    ) -> Result<ZoneType, ZoneTyperError> {
        let country_rules = self
            .countries_rules
            .get(country_code)
            .ok_or_else(|| ZoneTyperError::InvalidCountry(country_code.to_string()))?;
        country_rules
            .get_zone_type(zone, zone_inclusions, all_zones)
            .ok_or_else(|| ZoneTyperError::UnkownLevel(zone.admin_level, country_code.to_string()))
    }

    pub fn contains_rule(&self, country_code: &str) -> bool {
        self.countries_rules.contains_key(country_code)
    }
}

impl CountryAdminTypeRules {
    /// Find the type of a zone using libpostal's rules
    ///
    /// First we look if there is a specific rule for the zone,
    /// else we take the default osm's admin_level rule
    fn get_zone_type(
        &self,
        zone: &Zone,
        zone_inclusions: &[ZoneIndex],
        all_zones: &[Zone],
    ) -> Option<ZoneType> {
        let overrides = self
            .overrides
            .get_overrided_type(zone, zone_inclusions, all_zones);
        match overrides {
            Some(o) => o,
            None => self
                .type_by_level
                .get(&zone.admin_level.unwrap_or(0).to_string())
                .cloned(),
        }
    }
}

impl RulesOverrides {
    /// find the overrided type if it exists
    ///
    /// This returns an Option<Option<ZoneType>>:
    /// Some(val) => if we have a specific rule for the zone (and val can be None, this is a way for libpostal to explicitly not type some zones)
    /// None => We have no specific rule for the zone
    fn get_overrided_type(
        &self,
        zone: &Zone,
        zone_inclusions: &[ZoneIndex],
        all_zones: &[Zone],
    ) -> Option<Option<ZoneType>> {
        // check id overrides
        let id_overrides = self.id_rules.get(&zone.osm_id);
        // if there is no override for this specific object, we check the contained_by overrides
        match id_overrides {
            Some(overrides) => Some(*overrides),
            None => {
                if self.contained_by.is_empty() {
                    return None;
                }
                let mut parents_osm_id = zone_inclusions
                    .iter()
                    .map(|idx| &all_zones[idx.index].osm_id);

                parents_osm_id
                    .find_map(|parent_osm_id| self.contained_by.get(parent_osm_id))
                    .and_then(|country_rules| {
                        country_rules
                            .get_zone_type(zone, zone_inclusions, all_zones)
                            .map(Some)
                    })
            }
        }
    }
}

fn read_libpostal_yaml_folder() -> Result<BTreeMap<String, CountryAdminTypeRules>, Error> {
    Ok(LIBPOSTAL_RULES_DIR.files().iter().filter_map(|d| {
        let contents = d.contents_utf8()?;
        let deserialized_level = read_libpostal_yaml(contents)
            .map_err(|e| {
                warn!(
                    "Levels corresponding to file: {:?} have been skipped due to {}",
                    d.path(),
                    e
                )
            })
            .ok()?;
        let country_code = d
            .path()
            .file_stem()
            .and_then(|f| f.to_str())
            .map(|f| f.to_string())
            .ok_or_else(|| {
                warn!(
                    "Levels corresponding to file: {:?} have been skipped, impossible to deduce country code",
                    d.path()
                )
            })
            .ok()?;

        Some((country_code.to_uppercase(), deserialized_level))
    }).collect())
}

fn read_libpostal_yaml(contents: &str) -> Result<CountryAdminTypeRules, Error> {
    Ok(serde_yaml::from_str(contents)?)
}

// stuff used for serde
// to simplify serde, we use a strcut mapping exactly the file schema
// and this struct is transformed to RulesOverrides with the 'From' trait
#[derive(Deserialize, Default, Debug)]
struct SerdeRulesOverrides {
    #[serde(default)]
    contained_by: BTreeMap<OsmPrimaryObjects, BTreeMap<String, CountryAdminTypeRules>>,
    #[serde(rename = "id", default)]
    id_rules: BTreeMap<OsmPrimaryObjects, BTreeMap<String, Option<ZoneType>>>,
}

impl From<SerdeRulesOverrides> for RulesOverrides {
    fn from(serde: SerdeRulesOverrides) -> RulesOverrides {
        let c = serde
            .contained_by
            .into_iter()
            .flat_map(|(osm_type, map)| {
                map.into_iter().map(move |(osm_id, rules)| {
                    (format!("{}:{}", osm_type.to_string(), osm_id), rules)
                })
            })
            .collect();
        let i = serde
            .id_rules
            .into_iter()
            .flat_map(|(osm_type, map)| {
                map.into_iter().map(move |(osm_id, rules)| {
                    (format!("{}:{}", osm_type.to_string(), osm_id), rules)
                })
            })
            .collect();
        RulesOverrides {
            contained_by: c,
            id_rules: i,
        }
    }
}

fn de_with_from<'de, D, T, U>(de: D) -> Result<U, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
    U: From<T>,
{
    T::deserialize(de).map(U::from)
}

#[cfg(test)]
mod test {
    use super::CountryAdminTypeRules;
    use crate::zone_typer::read_libpostal_yaml;
    use cosmogony::{Zone, ZoneIndex, ZoneType};

    #[test]
    fn test_read_libpostal_yaml_basic() {
        let yaml_basic = r#"---
    admin_level: 
        "3": "country"
        "7": "state"
        "5": "city_district"
        "8": "city""#;

        let deserialized_levels = read_libpostal_yaml(yaml_basic).expect("invalid yaml");

        assert_eq!(
            deserialized_levels
                .type_by_level
                .get(&"3".to_string())
                .unwrap(),
            &ZoneType::Country
        );

        assert_eq!(
            deserialized_levels
                .type_by_level
                .get(&"5".to_string())
                .unwrap(),
            &ZoneType::CityDistrict
        );
    }

    /// Ensure that read_libpostal_yaml() returns an error when the yaml file is not valid.
    /// Specifically here the indentation of the "overrides" field is not ok.
    #[test]
    fn test_read_libpostal_yaml_ko() {
        let yaml_ko = r#"---
    admin_level: 
        "3": "country"
        "7": "state"
        "8": "city"

        overrides:
            contained_by:
                relation:
                    "5829526":
                        admin_level:
                            "10": "suburb""#;

        let deserialized_levels = read_libpostal_yaml(yaml_ko);

        assert!(deserialized_levels.is_err());
    }

    #[test]
    fn test_read_libpostal_contained_overrides() {
        let yaml = r#"---
    admin_level:
        "2": "country"
        "4": "state"
        "5": "state_district"
        "6": "state_district"
        "8": "city"
        "9": "suburb"

    overrides:
        contained_by:
            relation:
                # Luxembourg City
                "407489":
                    admin_level:
                        "9": "city_district""#;
        let deserialized_levels = read_libpostal_yaml(yaml).expect("invalid yaml");

        assert_eq!(
            deserialized_levels
                .type_by_level
                .get(&"2".to_string())
                .unwrap(),
            &ZoneType::Country
        );

        assert_eq!(
            deserialized_levels
                .overrides
                .contained_by
                .get(&"relation:407489".to_string())
                .unwrap()
                .type_by_level
                .get(&"9".to_string())
                .unwrap(),
            &ZoneType::CityDistrict
        );
    }

    #[test]
    fn test_read_libpostal_id_overrides() {
        let yaml = r#"---
    admin_level:
        "2": "country"
        "4": "state"
        "5": "state_district"
        "6": "state_district"
        "8": "city"
        "9": "suburb"

    overrides:
        id:
            relation:
                "1803923": "city_district"
                "42": null # it is a way in libpostal to remove a zone from being typed
                "#;
        let deserialized_levels = read_libpostal_yaml(yaml).expect("invalid yaml");

        assert_eq!(
            deserialized_levels
                .type_by_level
                .get(&"2".to_string())
                .unwrap(),
            &ZoneType::Country
        );

        assert_eq!(
            deserialized_levels
                .overrides
                .id_rules
                .get(&"relation:1803923".to_string())
                .unwrap(),
            &Some(ZoneType::CityDistrict)
        );

        assert_eq!(
            deserialized_levels
                .overrides
                .id_rules
                .get(&"relation:42".to_string())
                .unwrap(),
            &None
        );
    }

    /// test reading all the libpostal files
    #[test]
    fn test_read_all_libpostal_files() {
        // there should be no error while reading a file
        let rules = super::read_libpostal_yaml_folder().unwrap();

        // we should have been able to read all the files
        // Note: this value can change if libpostal add/remove some rules
        assert_eq!(rules.len(), 242);
    }

    /// helper method to return a yaml with many corner cases
    fn complex_rules() -> CountryAdminTypeRules {
        let yaml = r#"---
    admin_level:
        "2": "country"
        "4": "state"
        "5": "state_district"
        "6": "state_district"
        "8": "city"
        "9": "suburb"

    overrides:
        id:
            relation:
                "z1": "city_district"
                "z4": null # it is a way in libpostal to remove a zone from being typed
                "z5": "city_district"

        contained_by:
            relation:
                "big_zone":
                    admin_level:
                        "9": "suburb"
                "#;
        read_libpostal_yaml(yaml).expect("invalid yaml")
    }

    #[test]
    fn simple_get_zone_type_test() {
        let rules = complex_rules();

        let mut idx = 0usize;
        let mut make_zone = |id: &str, lvl| {
            let z = Zone {
                id: ZoneIndex { index: idx },
                osm_id: format!("relation:{}", id.to_string()),
                admin_level: lvl,
                ..Default::default()
            };
            idx += 1;
            z
        };
        let zones = vec![
            make_zone("z1", None),
            make_zone("z2", Some(5)),
            make_zone("z3", Some(9)),
            make_zone("z4", Some(9)),
            make_zone("z5", Some(7)),
            make_zone("z6", Some(7)),
            make_zone("big_zone", Some(4)),
            make_zone("very_big_zone", Some(2)),
        ];

        let mut inclusions = vec![vec![]; zones.len()];
        {
            let mut included_by = |z_osm_id, parents_id: Vec<&str>| {
                let find_zone_id = |osm_id: &str| {
                    zones
                        .iter()
                        .find(|z| z.osm_id == format!("relation:{}", osm_id))
                        .unwrap()
                        .id
                };
                inclusions[find_zone_id(z_osm_id).index] =
                    parents_id.into_iter().map(&find_zone_id).collect();
            };
            included_by("z1", vec!["big_zone"]);
            included_by("z2", vec!["big_zone"]);
            included_by("z3", vec!["very_big_zone", "big_zone"]);
            included_by("z4", vec!["big_zone"]);
            included_by("z5", vec![]);
            included_by("z6", vec![]);
            included_by("big_zone", vec![]);
            included_by("very_big_zone", vec![]);
        }

        let get_zone_type = |osm_id: &str| {
            let z = zones
                .iter()
                .find(|z| z.osm_id == format!("relation:{}", osm_id))
                .unwrap();
            rules.get_zone_type(z, &inclusions[z.id.index], &zones)
        };

        // even if z1 has no admin_level it has explicitly been set by libpostal to city_district
        assert_eq!(get_zone_type("z1"), Some(ZoneType::CityDistrict));

        // z2 is contained by 'big_zone' that has some overrides, but they do not concerns z2, so the default apply (5 -> StateDistrict)
        assert_eq!(get_zone_type("z2"), Some(ZoneType::StateDistrict));

        // z3 is contained by 'big_zone' that has a special rule for the admin_level 9
        assert_eq!(get_zone_type("z3"), Some(ZoneType::Suburb));

        // z4 has 2 conflicting override rules that can be applied:
        // contained by 'big_zone' and explicit Id override
        // in this case it's the 'id rule' that wins
        assert_eq!(get_zone_type("z4"), None);

        // z5 has a simple override by id
        assert_eq!(get_zone_type("z5"), Some(ZoneType::CityDistrict));

        // z6 has no override, but it's level is not mapped
        assert_eq!(get_zone_type("z6"), None);

        // no specific stuff for big_zone and very_big zone
        assert_eq!(get_zone_type("big_zone"), Some(ZoneType::State));
        assert_eq!(get_zone_type("very_big_zone"), Some(ZoneType::Country));
    }
}
