pub struct Guard<'a> {
    body: &'a u8,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        let _ = self.body;
    }
}

fn hold(body: &u8) -> Guard<'_> {
    Guard { body }
}
