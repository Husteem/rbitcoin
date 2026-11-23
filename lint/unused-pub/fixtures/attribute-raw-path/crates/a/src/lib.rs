pub fn encode() {}

struct Row {
    #[serde(serialize_with = r"encode")]
    field: u8,
}
