#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
//#[serde(rename_all = "camelCase")]   // camelCase not working for underscores => snake enum
pub enum AdminType {
    state,
    state_district,
    country,
    country_region,
    city,
    city_district,
    suburb,
    NonAdministrative,
}
