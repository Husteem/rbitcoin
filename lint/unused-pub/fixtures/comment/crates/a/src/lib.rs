pub fn gone() {}

// gone() used to run here.
/// See [`gone`].
fn other() -> &'static str {
    "gone"
}
