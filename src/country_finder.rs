use crate::zone_typer::ZoneTyper;
use cosmogony::{Zone, ZoneIndex};
use std::collections::BTreeMap;

pub const COUNTRY_CODE_TAG: &str = "ISO3166-1:alpha2";

// to reduce the memory footprint we only store some of the countries information
pub struct Country {
    iso: String, // ISO3166-1:alpha2 code (eg: FR, DE, US, etc.),
    admin_level: Option<u32>,
}

pub struct CountryFinder {
    countries: BTreeMap<ZoneIndex, Country>,
}

impl Default for CountryFinder {
    fn default() -> Self {
        CountryFinder {
            countries: BTreeMap::new(),
        }
    }
}

impl CountryFinder {
    pub fn init(zones: &[Zone], typer: &ZoneTyper) -> Self {
        CountryFinder {
            countries: zones
                .into_iter()
                .filter_map(|z| {
                    z.tags
                        .get(COUNTRY_CODE_TAG) // iso3166 code, should use capital letters
                        .map(|country_code| country_code.to_uppercase())
                        .filter(|country_code| typer.contains_rule(country_code))
                        .map(|country_code| {
                            (
                                z.id.clone(),
                                Country {
                                    iso: country_code.clone(),
                                    admin_level: z.admin_level,
                                },
                            )
                        })
                })
                .collect(),
        }
    }

    pub fn find_zone_country(&self, z: &Zone, inclusion: &[ZoneIndex]) -> Option<String> {
        inclusion
            .iter()
            .chain(std::iter::once(&z.id)) // we also add the zone to check if it's itself a country
            .filter_map(|parent_index| self.countries.get(&parent_index))
            .max_by_key(|c| c.admin_level.unwrap_or(0u32))
            .map(|c| c.iso.clone())
    }

    pub fn is_empty(&self) -> bool {
        self.countries.is_empty()
    }
}
