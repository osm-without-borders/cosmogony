use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone)]
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

impl FromStr for AdminType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.as_ref() {
            "country" => Ok(AdminType::Country),
            "country_region" => Ok(AdminType::CountryRegion),
            "state" => Ok(AdminType::State),
            "state_district" => Ok(AdminType::StateDistrict),
            "city" => Ok(AdminType::City),
            "city_district" => Ok(AdminType::CityDistrict),
            "suburb" => Ok(AdminType::Suburb),
            &_ => Err(()),
        }
    }
}
