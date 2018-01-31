use std::collections::BTreeMap;
use zone::{Zone, ZoneType};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::io::prelude::*;
use failure::Error;
use serde_yaml;
use failure::ResultExt;

#[derive(Debug)]
pub struct ZoneTyper {
    countries_rules: BTreeMap<String, CountryAdminTypeRules>,
}

#[derive(Serialize, Deserialize, Debug)]
struct CountryAdminTypeRules {
    pub admin_level: BTreeMap<String, ZoneType>,
    // WIP
    //#[serde(default)]
    //pub overrides: Option<Overrides>,
}

impl ZoneTyper {
    pub fn create(libpostal_files_path: PathBuf) -> Result<ZoneTyper, Error> {
        Ok(ZoneTyper {
            countries_rules: read_libpostal_yaml_folder(libpostal_files_path)?,
        })
    }

    pub fn get_zone_type(zone: &Zone, country_code: &str) -> ZoneType {
        panic!("")
    }
}

// WIP
//#[derive(Serialize, Deserialize, Debug)]
//pub struct Overrides {
//    #[serde(default)]
//    pub id: Option<Id>,
//}
//
//#[derive(Serialize, Deserialize, Debug)]
//pub struct Id {
//    #[serde(default)]
//    pub relation: BTreeMap<String, String>,
//}

fn read_libpostal_yaml_folder(
    yaml_files_folder: PathBuf,
) -> Result<BTreeMap<String, CountryAdminTypeRules>, Error> {
    let mut admin_levels: BTreeMap<String, CountryAdminTypeRules> = BTreeMap::new();
    let paths = fs::read_dir(&yaml_files_folder).context(format!(
        "error while reading libpostal directory {:?}",
        yaml_files_folder
    ))?;
    for entry in paths {
        let mut contents = String::new();

        if let Ok(a_path) = entry {
            if let Ok(mut f) = fs::File::open(&a_path.path()) {
                if let Ok(_) = f.read_to_string(&mut contents) {
                    let deserialized_level = match read_libpostal_yaml(&contents) {
                        Ok(a) => a,
                        Err(_) => {
                            warn!(
                                "Levels corresponding to file: {:?} have been skipped",
                                &a_path.path()
                            );
                            continue;
                        }
                    };

                    let country_code = match a_path
                        .path()
                        .file_name()
                        .and_then(|f| f.to_str())
                        .map(|f| f.to_string())
                    {
                        Some(name) => name.into(),
                        None => {
                            warn!(
                                "Levels corresponding to file: {:?} have been skipped",
                                &a_path.path()
                            );
                            continue;
                        }
                    };

                    admin_levels.insert(country_code, deserialized_level);
                };
            }
        }
    }

    Ok(admin_levels)
}

fn read_libpostal_yaml(contents: &String) -> Result<CountryAdminTypeRules, Error> {
    Ok(serde_yaml::from_str(&contents)?)
}

#[cfg(test)]
mod test {
    use zone_typer::read_libpostal_yaml;
    use zone::ZoneType;

    #[test]
    fn test_read_libpostal_yaml_basic() {
        let yaml_basic = r#"---
    admin_level: 
        "3": "country"
        "7": "state"
        "5": "city_district"
        "8": "city""#.to_string();

        let deserialized_levels = read_libpostal_yaml(&yaml_basic).expect("invalid yaml");

        assert_eq!(
            deserialized_levels
                .admin_level
                .get(&"3".to_string())
                .unwrap(),
            &ZoneType::Country
        );

        assert_eq!(
            deserialized_levels
                .admin_level
                .get(&"5".to_string())
                .unwrap(),
            &ZoneType::CityDistrict
        );
    }

    // WIP...
    //#[test]
    //fn test_read_libpostal_yaml_overrides() {
    //    let yaml_overrides = r#"---
    //    admin_level:
    //        "3": "country"
    //        "7": "state"
    //        "8": "city"
    //    overrides:
    //        id:
    //            relation:
    //                 "5829526": "city""#.to_string();
    //
    //    let deserialized_levels = cosmogony::read_libpostal_yaml(&yaml_overrides).expect("invalid yaml");
    //
    //    let id = deserialized_levels
    //        .overrides
    //        .expect("overrides problem")
    //        .id
    //        .expect("id problem");
    //
    //    assert_eq!(
    //        id.relation.get(&"5829526".to_string()),
    //        Some(&"city".to_string())
    //    );
    //}

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
                            "10": "suburb""#.to_string();

        let deserialized_levels = read_libpostal_yaml(&yaml_ko);

        assert_eq!(deserialized_levels.is_err(), true);
    }

}
