pub trait Seal<const N: usize> {}
pub trait Borrowed {}

impl<const N: usize> Seal<N> for [u8; N] {}
impl<T> Borrowed for &T {}
