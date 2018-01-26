use std::fmt;
use std::io::Read;
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Deserialize, Debug)]
pub struct Country {
    pub admin_level: BTreeMap<String, String>,
}
