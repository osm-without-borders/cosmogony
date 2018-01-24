#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AdminType {
    NonAdministrative,
    City,
    Country,
}
