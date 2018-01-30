#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
//#[serde(rename_all = "camelCase")]   // camelCase not working for underscores => snake enum
pub enum AdminType {
    #[serde(rename = "state")]
    State,
    #[serde(rename = "state_district")]
    StateDistrict,
    #[serde(rename = "country")]
    Country,
    #[serde(rename = "country_region")]
    CountryRegion,
    #[serde(rename = "city")]
    City,
    #[serde(rename = "city_district")]
    CityDistrict,
    #[serde(rename = "suburb")]
    Suburb,
    NonAdministrative,
}
