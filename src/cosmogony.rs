use std::collections::BTreeMap;
use zone::Zone;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Cosmogony {
    pub zones: Vec<Zone>,
    pub meta: CosmogonyMetadata,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CosmogonyMetadata {
    pub osm_filename: String,
    pub stats: CosmogonyStats,
    // errors:
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CosmogonyStats {
    pub level_counts: BTreeMap<u32, u64>,
    pub wikidata_counts: BTreeMap<u32, u64>,
}
