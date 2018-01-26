use std::collections::{BTreeMap};

#[derive(Serialize, Deserialize, Debug)]
pub struct Country {
    pub admin_level: BTreeMap<String, String>,
}
