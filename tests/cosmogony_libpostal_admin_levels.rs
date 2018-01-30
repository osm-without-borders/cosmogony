extern crate cosmogony;

use std::collections::BTreeMap;
use admin_type::AdminType;  // problem: cannot refer to admin_type

#[test]
fn test_read_libpostal_yaml_basic() {
    let yaml_basic = r#"---
    admin_level: 
        "3": "country"
        "7": "state"
        "8": "city""#.to_string();

    let deserialized_levels = cosmogony::read_libpostal_yaml(&yaml_basic).expect("invalid yaml");

    assert_eq!(
        deserialized_levels.admin_level.get(&"3".to_string()),
        Some(AdminType::Country)
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

#[test]
fn test_read_libpostal_yaml_ko() {
    // Ensure that read_libpostal_yaml() returns an error when the yaml file is not valid.
    // Specifically here the indentation of the "overrides" field is not ok.
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

    let deserialized_levels = cosmogony::read_libpostal_yaml(&yaml_ko);

    assert_eq!(deserialized_levels.is_err(), true);
}
