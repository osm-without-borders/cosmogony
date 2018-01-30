#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AdminType {
    NonAdministrative,
    City,
    Country,
    CountryRegion,
    State,
    StateDistrict,
    CityDistrict,
    Suburb,
}
