pub struct Left;
pub struct Right;

impl Left {
    pub fn open() {}
}

impl Right {
    pub fn open() {}
}

fn caller() {
    let _ = Right;
    Left::open();
}
