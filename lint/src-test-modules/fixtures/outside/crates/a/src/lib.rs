#[cfg(debug_assertions)]
fn hook() {}

#[cfg(feature = "mutants")]
fn skip_mutants() {}
