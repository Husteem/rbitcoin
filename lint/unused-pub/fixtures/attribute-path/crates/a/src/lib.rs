pub fn ser_owned() {}

struct Row {
    #[serde(serialize_with = "ser_owned")]
    field: u8,
}
