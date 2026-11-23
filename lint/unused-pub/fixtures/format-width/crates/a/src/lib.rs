pub const WIDTH: usize = 3;
pub const DIGITS: usize = 2;

fn padded() -> String {
    format!("{:WIDTH$} {:.DIGITS$}", 1, 1.0)
}
