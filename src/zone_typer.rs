use failure::ResultExt;
use failure::{err_msg, Error};
use serde_yaml;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs;
use std::io::prelude::*;
use std::path::Path;
use zone::{Zone, ZoneType};

#[derive(Debug)]
pub struct ZoneTyper {
    countries_rules: BTreeMap<String, CountryAdminTypeRules>,
}

#[derive(Serialize, Deserialize, Debug)]
struct CountryAdminTypeRules {
    #[serde(rename = "admin_level")]
    pub type_by_level: BTreeMap<String, ZoneType>,
}

#[derive(Debug, Fail)]
pub enum ZoneTyperError {
    #[fail(display = "impossible to find country {}", _0)]
    InvalidCountry(String),
    #[fail(display = "no lvl {:?} in libpostal rule for {}", _0, _1)]
    UnkownLevel(Option<u32>, String),
}

impl ZoneTyper {
    pub fn new<T>(libpostal_files_path: T) -> Result<ZoneTyper, Error>
    where
        T: AsRef<Path> + Debug,
    {
        let z = ZoneTyper {
            countries_rules: read_libpostal_yaml_folder(&libpostal_files_path)?,
        };
        if z.countries_rules.is_empty() {
            Err(err_msg(format!(
                "no country rules have been loaded, the directory {:?} \
                 must contains valid libpostal rules",
                &libpostal_files_path
            )))
        } else {
            Ok(z)
        }
    }

    pub fn get_zone_type(
        &self,
        zone: &Zone,
        country_code: &str,
    ) -> Result<ZoneType, ZoneTyperError> {
        let country_rules = self.countries_rules
            .get(&country_code.to_lowercase()) // file postal code are lowercase
            .ok_or(ZoneTyperError::InvalidCountry(country_code.to_string()))?;
        Ok(country_rules
            .type_by_level
            .get(&zone.admin_level.unwrap_or(0).to_string())
            .ok_or(ZoneTyperError::UnkownLevel(
                zone.admin_level.clone(),
                country_code.to_string(),
            ))?
            .clone())
    }
}

fn read_libpostal_yaml_folder<T>(
    yaml_files_folder: T,
) -> Result<BTreeMap<String, CountryAdminTypeRules>, Error>
where
    T: AsRef<Path> + Debug,
{
    use std::fs::DirEntry;

    let read_libpostal_file = |entry: Result<DirEntry, _>| {
        let a_path = entry.ok()?;
        let mut f = fs::File::open(&a_path.path()).ok()?;
        let mut contents = String::new();
        f.read_to_string(&mut contents).ok()?;
        let deserialized_level = read_libpostal_yaml(&contents)
            .map_err(|_| {
                warn!(
                    "Levels corresponding to file: {:?} have been skipped",
                    &a_path.path()
                )
            })
            .ok()?;
        let country_code = a_path
            .path()
            .file_stem()
            .and_then(|f| f.to_str())
            .map(|f| f.to_string())
            .ok_or_else(|| {
                warn!(
                    "Levels corresponding to file: {:?} have been skipped",
                    &a_path.path()
                )
            })
            .ok()?;

        Some((country_code, deserialized_level))
    };

    Ok(fs::read_dir(&yaml_files_folder)
        .context(format!(
            "error while reading libpostal directory {:?}",
            yaml_files_folder
        ))?
        .filter_map(read_libpostal_file)
        .collect())
}

fn read_libpostal_yaml(contents: &str) -> Result<CountryAdminTypeRules, Error> {
    Ok(serde_yaml::from_str(&contents)?)
}

#[cfg(test)]
mod test {
    use zone::ZoneType;
    use zone_typer::read_libpostal_yaml;

    #[test]
    fn test_read_libpostal_yaml_basic() {
        let yaml_basic = r#"---
    admin_level: 
        "3": "country"
        "7": "state"
        "5": "city_district"
        "8": "city""#;

        let deserialized_levels = read_libpostal_yaml(&yaml_basic).expect("invalid yaml");

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

        let deserialized_levels = read_libpostal_yaml(&yaml_ko);

        assert_eq!(deserialized_levels.is_err(), true);
    }

}
