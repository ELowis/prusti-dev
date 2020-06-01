//! Data structures for storing information about specifications.
//!
//! Please see the `parser.rs` file for more information about
//! specifications.

use std::convert::TryFrom;
use std::string::ToString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// A specification type.
pub enum SpecType {
    /// Precondition of a procedure.
    Precondition,
    /// Postcondition of a procedure.
    Postcondition,
    /// Loop invariant or struct invariant
    Invariant,
}

#[derive(Debug)]
/// A conversion from string into specification type error.
pub enum TryFromStringError {
    /// Reported when the string being converted is not one of the
    /// following: `requires`, `ensures`, `invariant`.
    UnknownSpecificationType,
}

impl<'a> TryFrom<&'a str> for SpecType {
    type Error = TryFromStringError;

    fn try_from(typ: &str) -> Result<SpecType, TryFromStringError> {
        match typ {
            "requires" => Ok(SpecType::Precondition),
            "ensures" => Ok(SpecType::Postcondition),
            "invariant" => Ok(SpecType::Invariant),
            _ => Err(TryFromStringError::UnknownSpecificationType),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone, Copy)]
/// A unique ID of the specification element such as entire precondition
/// or postcondition.
pub struct SpecID(u64);

impl SpecID {
    /// Constructor.
    pub fn new() -> Self {
        Self { 0: 100 }
    }
    /// Increment ID and return a copy of the new value.
    pub fn inc(&mut self) -> Self {
        self.0 += 1;
        Self { 0: self.0 }
    }
}

impl ToString for SpecID {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl From<u64> for SpecID {
    fn from(value: u64) -> Self {
        Self { 0: value }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone, Copy)]
/// A unique ID of the Rust expression used in the specification.
pub struct ExpressionId(usize);

impl ExpressionId {
    /// Constructor.
    pub fn new() -> Self {
        Self { 0: 100 }
    }
    /// Increment ID and return a copy of the new value.
    pub fn inc(&mut self) -> Self {
        self.0 += 1;
        Self { 0: self.0 }
    }
}

impl ToString for ExpressionId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Into<usize> for ExpressionId {
    fn into(self) -> usize {
        self.0
    }
}

impl Into<u128> for ExpressionId {
    fn into(self) -> u128 {
        self.0 as u128
    }
}

impl From<u128> for ExpressionId {
    fn from(value: u128) -> Self {
        Self { 0: value as usize }
    }
}

#[derive(Debug, Clone)]
/// A Rust expression used in the specification.
pub struct Expression<ET> {
    /// Unique identifier.
    pub id: ExpressionId,
    /// Actual expression.
    pub expr: ET,
}

#[derive(Debug, Clone)]
/// An assertion used in the specification.
pub struct Assertion<ET, AT> {
    /// Subassertions.
    pub kind: Box<AssertionKind<ET, AT>>,
}

#[derive(Debug, Clone)]
/// A single trigger for a quantifier.
pub struct Trigger<ET>(Vec<Expression<ET>>);

impl<ET> Trigger<ET> {
    /// Construct a new trigger, which is a “conjunction” of terms.
    pub fn new(terms: Vec<Expression<ET>>) -> Trigger<ET> {
        Trigger(terms)
    }
    /// Getter for terms.
    pub fn terms(&self) -> &Vec<Expression<ET>> {
        &self.0
    }
}

impl<ET> IntoIterator for Trigger<ET> {
    type Item = Expression<ET>;
    type IntoIter = ::std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Clone)]
/// A set of triggers used in the quantifier.
pub struct TriggerSet<ET>(Vec<Trigger<ET>>);

impl<ET> TriggerSet<ET> {
    /// Construct a new trigger set.
    pub fn new(triggers: Vec<Trigger<ET>>) -> TriggerSet<ET> {
        TriggerSet(triggers)
    }
    /// Getter for triggers.
    pub fn triggers(&self) -> &Vec<Trigger<ET>> {
        &self.0
    }
}

impl<ET> IntoIterator for TriggerSet<ET> {
    type Item = Trigger<ET>;
    type IntoIter = ::std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Clone)]
/// A sequence of variables used in the forall.
pub struct ForAllVars<AT> {
    /// Unique id for this sequence of variables.
    pub id: ExpressionId,
    /// Variables.
    pub vars: Vec<AT>,
}

#[derive(Debug, Clone)]
/// An assertion kind used in the specification.
pub enum AssertionKind<ET, AT> {
    /// A single Rust expression.
    Expr(Expression<ET>),
    /// Conjunction &&.
    And(Vec<Assertion<ET, AT>>),
    /// Implication ==>
    Implies(Assertion<ET, AT>, Assertion<ET, AT>),
    /// TODO < Even > ==> x % 2 == 0
    TypeCond(ForAllVars<AT>, Assertion<ET, AT>),
    /// Quantifier (forall vars :: {triggers} filter ==> body)
    ForAll(ForAllVars<AT>, TriggerSet<ET>, Assertion<ET, AT>),
    /// Pledge after_expiry<reference>(rhs)
    ///     or after_expiry_if<reference>(lhs,rhs)
    Pledge(
        /// The blocking reference used in a loop. None for postconditions.
        Option<Expression<ET>>,
        /// The body lhs.
        Assertion<ET, AT>,
        /// The body rhs.
        Assertion<ET, AT>,
    ),
}

#[derive(Debug, Clone)]
/// Specification such as precondition, postcondition, or invariant.
pub struct Specification<ET, AT> {
    /// Specification type.
    pub typ: SpecType,
    /// Actual specification.
    pub assertion: Assertion<ET, AT>,
}

#[derive(Debug, Clone)]
/// Specification of a single element such as procedure or loop.
pub enum SpecificationSet<ET, AT> {
    /// (Precondition, Postcondition)
    Procedure(Vec<Specification<ET, AT>>, Vec<Specification<ET, AT>>),
    /// Loop invariant.
    Loop(Vec<Specification<ET, AT>>),
    /// Struct invariant.
    Struct(Vec<Specification<ET, AT>>),
}

impl<ET, AT> SpecificationSet<ET, AT> {
    pub fn is_empty(&self) -> bool {
        match self {
            SpecificationSet::Procedure(ref pres, ref posts) => pres.is_empty() && posts.is_empty(),
            SpecificationSet::Loop(ref invs) => invs.is_empty(),
            SpecificationSet::Struct(ref invs) => invs.is_empty(),
        }
    }
}