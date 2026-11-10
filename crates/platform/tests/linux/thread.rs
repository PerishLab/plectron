#![cfg(test)]

use super::*;

trait Affine<Marker> {
    fn affine() {}
}

impl<T: ?Sized> Affine<()> for T {}

struct Sendable;

impl<T: ?Sized + Send> Affine<Sendable> for T {}

#[test]
fn thread() {
    <Connection as Affine<_>>::affine();
    <Desktop<Error> as Affine<_>>::affine();
}
