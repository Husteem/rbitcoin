pub struct Guard<'a> {
    body: &'a u8,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        let _ = self.body;
    }
}
