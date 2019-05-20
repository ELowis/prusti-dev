// © 2019, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Optimisation that removes unused temporary variables.

use encoder::vir::cfg;
use encoder::vir::{Const, Expr, Stmt};
use std::collections::HashSet;
use std::mem;

/// Remove trivial assertions:
/// * `assert true`
/// * `exhale true`
pub fn remove_trivial_assertions(mut method: cfg::CfgMethod) -> cfg::CfgMethod {
    method.retain_stmts(|stmt| {
        // Remove those statements marked with `false`
        match stmt {
            Stmt::Assert(Expr::Const(Const::Bool(true), _), _) => false,
            Stmt::Exhale(Expr::Const(Const::Bool(true), _), _) => false,
            _ => true, // Keep the rest
        }
    });
    method
}
