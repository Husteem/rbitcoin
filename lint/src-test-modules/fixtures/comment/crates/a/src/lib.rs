// #[cfg(test)] in a line comment.
/*
#[cfg(test)]
mod tests {}
*/
const NOTE: &str = "#[cfg(test)]";

#[cfg(debug_assertions)]
fn hook() {}
