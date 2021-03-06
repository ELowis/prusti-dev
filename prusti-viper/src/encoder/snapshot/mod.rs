// © 2021, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use rustc_middle::ty;
use prusti_common::vir;
use prusti_common::vir::{Expr, Type};
use std::collections::HashMap;

pub mod encoder;
mod patcher;

/// Snapshot of a VIR type. This enum is internal to the snapshot encoding and
/// should not need to be exposed to the encoder in general.
#[derive(Debug, Clone)]
enum Snapshot {
    /// Corresponds directly to an existing Viper type.
    Primitive(Type),
    /// Encodes types with no content; these need not be provided as arguments
    /// to snapshot constructors.
    Unit,
    /// Encodes a complex type: tuples, ADTs, or closures.
    Complex {
        predicate_name: String,
        domain: vir::Domain,
        discriminant_func: vir::DomainFunc,
        snap_func: vir::Function,
        /// [variants] has one entry for tuples, structs, and closures.
        /// For enums, it has as many entries as there are variants.
        /// The first function is the constructor, the hashmap encodes the
        /// field access functions, keyed by their name.
        variants: Vec<(vir::DomainFunc, HashMap<String, vir::DomainFunc>)>,
        /// Mapping of variant names (as used by Prusti) to variant indices
        /// in the [variants] vector. Empty for non-enums.
        variant_names: HashMap<String, usize>,
    }, // TODO: separate variant for enums and one-variant Complexes?
    /// Type cannot be encoded: type parameters, unsupported types.
    Abstract {
        predicate_name: String,
        domain: vir::Domain,
        snap_func: vir::Function,
    },

    /// A type which will be resolved to a different snapshot kind.
    /// (Should only appear while encoding is in progress!)
    Lazy(Type),
}

impl Snapshot {
    pub fn get_type(&self) -> Type {
        match self {
            Self::Primitive(ty) => ty.clone(),
            Self::Unit => Type::Domain(encoder::UNIT_DOMAIN_NAME.to_string()),
            Self::Complex { predicate_name, .. } => Type::Snapshot(predicate_name.to_string()),
            Self::Abstract { predicate_name, .. } => Type::Snapshot(predicate_name.to_string()),
            Self::Lazy(ty) => ty.clone(),
        }
    }

    pub fn is_quantifiable(&self) -> bool {
        // TODO: allow more types?
        match self {
            Self::Primitive(_) => true,
            _ => false,
        }
    }

    pub fn supports_equality(&self) -> bool {
        match self {
            Self::Primitive(_) => true,
            Self::Unit => true,
            Self::Complex { .. } => true,
            _ => false,
        }
    }
}
