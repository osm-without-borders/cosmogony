#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdminType {
    State,
    StateDistrict,
    Country,
    CountryRegion,
    City,
    CityDistrict,
    Suburb,
    NonAdministrative,
}
