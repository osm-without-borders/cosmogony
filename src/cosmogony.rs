use std::collections::BTreeMap;
use std::fmt;
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

impl fmt::Display for CosmogonyStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (level, count) in &self.level_counts {
            let wd = self.wikidata_counts.get(level).unwrap_or(&0u64);
            write!(f, "Admin level {}: {} elements\n", level, count)?;
            write!(f, "    {} with wikidata id\n", wd)?;
        }

        Ok(())
    }
}
