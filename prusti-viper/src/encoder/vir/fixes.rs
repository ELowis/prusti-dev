// © 2019, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Fix potentially broken encoding.

use super::ast;
use super::cfg;
use std::collections::HashSet;
use std::mem;

/// Viper has a consistency check that only variables declared inside
/// the package statement can be assigned in it. Since these ghost
/// variables are generated by method calls that happen outside the
/// package statement, it is hard to satisfy this requirement when
/// creating the encoding. Therefore, we fix this with an additional
/// pass that renames all variables declared inside package statements
/// so that they are unique.
pub fn fix_ghost_vars(mut method: cfg::CfgMethod) -> cfg::CfgMethod {
    let mut fixer = GhostVarFixer {
        package_stmt_count: 0,
        vars: None,
    };
    let mut sentinel_stmt = ast::Stmt::Comment(String::from("moved out stmt"));
    for block in &mut method.basic_blocks {
        for stmt in &mut block.stmts {
            mem::swap(&mut sentinel_stmt, stmt);
            sentinel_stmt = ast::StmtFolder::fold(&mut fixer, sentinel_stmt);
            mem::swap(&mut sentinel_stmt, stmt);
        }
    }
    method
}

struct GhostVarFixer {
    /// A counter that assigns a unique number to each package statement.
    package_stmt_count: u32,
    /// A set of variables declared inside a package stmt that should be
    /// renamed.
    vars: Option<HashSet<ast::LocalVar>>,
}

impl GhostVarFixer {
    fn fix_name(&self, mut local_var: ast::LocalVar) -> ast::LocalVar {
        local_var
            .name
            .push_str(&format!("$p{}", self.package_stmt_count));
        local_var
    }
}

impl ast::ExprFolder for GhostVarFixer {
    fn fold_local(&mut self, local_var: ast::LocalVar, pos: ast::Position) -> ast::Expr {
        match self.vars {
            Some(ref vars) if vars.contains(&local_var) => {
                ast::Expr::Local(self.fix_name(local_var), pos)
            }
            _ => ast::Expr::Local(local_var, pos),
        }
    }
}

impl ast::StmtFolder for GhostVarFixer {
    fn fold_expr(&mut self, e: ast::Expr) -> ast::Expr {
        ast::ExprFolder::fold(self, e)
    }

    fn fold_package_magic_wand(
        &mut self,
        wand: ast::Expr,
        body: Vec<ast::Stmt>,
        label: String,
        vars: Vec<ast::LocalVar>,
        pos: ast::Position,
    ) -> ast::Stmt {
        let wand = self.fold_expr(wand);
        self.vars = Some(vars.into_iter().collect());
        let body = body.into_iter().map(|stmt| self.fold(stmt)).collect();
        let unfixed_vars = self.vars.take().unwrap();
        let vars = unfixed_vars
            .into_iter()
            .map(|var| self.fix_name(var))
            .collect();
        self.package_stmt_count += 1;
        ast::Stmt::PackageMagicWand(wand, body, label, vars, pos)
    }
}
