use std::hash::Hash;

use super::TestArch;
use crate::{comp, Entity};

/// A test discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec_codegen::Discrim)]
#[dynec(dynec_as(crate))]
pub struct TestDiscrim1(pub(crate) usize);

/// An alternative test discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec_codegen::Discrim)]
#[dynec(dynec_as(crate))]
pub struct TestDiscrim2(pub(crate) usize);

/// Does not have auto init
#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim1)]
#[derive(Debug, Clone, PartialEq)]
pub struct IsoNoInit(pub i32);

/// Has auto init
#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim2, init = || IsoWithInit(73), required)]
#[derive(Debug, Clone, PartialEq)]
pub struct IsoWithInit(pub i32);

/// An isotope component with a strong reference to [`TestArch`].
#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim1)]
pub struct StrongRefIsotope(#[entity] pub Entity<TestArch>);
