use std::collections::{BTreeMap};

#[derive(Serialize, Deserialize, Debug)]
pub struct AdminRules {
    pub admin_level: BTreeMap<String, String>,
}
