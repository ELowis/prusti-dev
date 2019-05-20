// © 2019, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use super::borrowck::facts;
use super::place_set::PlaceSet;
use crate::utils;
use rustc::mir;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionKind {
    /// Gives read permission to this node. It must not be a leaf node.
    ReadNode,
    /// Gives read permission to the entire subtree including this node.
    /// This must be a leaf node.
    ReadSubtree,
    /// Gives write permission to this node. It must not be a leaf node.
    WriteNode,
    /// Gives write permission to the entire subtree rooted at this node.
    /// This must be a leaf node.
    WriteSubtree,
    /// Gives write permission to the entire subtree including this node.
    /// This must be a leaf node.
    WriteNodeAndSubtree,
    /// Give no permission to this node and the entire subtree. This
    /// must be a leaf node.
    None,
}

impl PermissionKind {
    pub fn is_none(&self) -> bool {
        match self {
            PermissionKind::None => true,

            PermissionKind::ReadNode
            | PermissionKind::ReadSubtree
            | PermissionKind::WriteNode
            | PermissionKind::WriteSubtree
            | PermissionKind::WriteNodeAndSubtree => false,
        }
    }
}

#[derive(Debug)]
pub struct Loan<'tcx> {
    /// ID used in Polonius.
    id: facts::Loan,
    /// The location where the borrow starts.
    location: mir::Location,
    /// The borrowed place.
    place: mir::Place<'tcx>,
}

//#[derive(Clone, Copy, Debug)]
//enum BorrowKind {
//Shared,
//Mutable,
//}

#[derive(Debug)]
pub enum PermissionNode<'tcx> {
    OwnedNode {
        place: mir::Place<'tcx>,
        kind: PermissionKind,
        children: Vec<PermissionNode<'tcx>>,
    },
    // TODO: Make this the type only of the root node.
    BorrowedNode {
        place: mir::Place<'tcx>,
        kind: PermissionKind,
        child: Box<PermissionNode<'tcx>>,
        /// A list of locations from where this borrow may be borrowing.
        // TODO: Is this needed?
        may_borrow_from: Vec<Loan<'tcx>>,
    },
}

impl<'tcx> PermissionNode<'tcx> {
    pub fn get_place(&self) -> &mir::Place<'tcx> {
        match self {
            PermissionNode::OwnedNode { place, .. } => place,
            PermissionNode::BorrowedNode { place, .. } => place,
        }
    }

    pub fn set_permission_kind(&mut self, permission_kind: PermissionKind) {
        match self {
            PermissionNode::OwnedNode { ref mut kind, .. } => {
                *kind = permission_kind;
            }
            PermissionNode::BorrowedNode { .. } => {
                unimplemented!();
            }
        }
    }

    pub fn get_permission_kind(&self) -> PermissionKind {
        match self {
            PermissionNode::OwnedNode { kind, .. } | PermissionNode::BorrowedNode { kind, .. } => {
                *kind
            }
        }
    }

    pub fn get_or_create_child(
        &mut self,
        place: &mir::Place<'tcx>,
        kind: PermissionKind,
    ) -> &mut Self {
        match self {
            PermissionNode::OwnedNode { children, .. } => {
                let index = children.iter().position(|child| child.get_place() == place);
                if let Some(index) = index {
                    return &mut children[index];
                }
                let child = PermissionNode::OwnedNode {
                    place: place.clone(),
                    kind: kind,
                    children: Vec::new(),
                };
                children.push(child);
                let len = children.len();
                &mut children[len - 1]
            }
            PermissionNode::BorrowedNode { .. } => {
                unimplemented!(); // TODO: Change code so that we do not
                                  // have to deal with this case.
            }
        }
    }

    pub fn get_children(&self) -> Vec<&PermissionNode<'tcx>> {
        match self {
            PermissionNode::OwnedNode { ref children, .. } => children.iter().collect(),

            PermissionNode::BorrowedNode { box ref child, .. } => vec![child],
        }
    }
}

impl<'tcx> fmt::Display for PermissionNode<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PermissionNode::OwnedNode {
                place,
                kind,
                children,
            } => {
                write!(f, "acc({:?}, {:?})", place, kind)?;
                for child in children.iter() {
                    write!(f, " && {}", child)?;
                }
            }
            PermissionNode::BorrowedNode { .. } => {
                unimplemented!();
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PermissionTree<'tcx> {
    root: PermissionNode<'tcx>,
}

#[derive(Debug, Clone, Copy)]
pub enum TargetType {
    /// Read a node or its contents.
    Read,
    /// Write the entire node.
    WriteNode,
    /// Write only the node's contents.
    WriteContents,
}

impl TargetType {
    fn is_write(self) -> bool {
        match self {
            TargetType::Read => false,
            TargetType::WriteNode | TargetType::WriteContents => true,
        }
    }
    fn to_permission_kind(self) -> PermissionKind {
        match self {
            TargetType::Read => PermissionKind::ReadSubtree,
            TargetType::WriteNode => PermissionKind::WriteNodeAndSubtree,
            TargetType::WriteContents => PermissionKind::WriteSubtree,
        }
    }
}

impl<'tcx> PermissionTree<'tcx> {
    /// Create a permission tree such that:
    ///
    /// +   The `place` is of kind `WriteSubtree` or `ReadSubtree`
    ///     depending on `target_type.is_write()`.
    /// +   All steps between `target_place` and `place` are of kind
    ///     `WriteNode` if `target_type.is_write()`.
    /// +   All steps from the root until `target_place` are of kind
    ///     `ReadNode`.
    pub fn new(
        place: &mir::Place<'tcx>,
        target_place: &mir::Place<'tcx>,
        target_type: TargetType,
    ) -> Self {
        let place = utils::VecPlace::new(place);
        let mut place_iter = place.iter().rev();
        let node_permission_kind = target_type.to_permission_kind();
        let mut node = PermissionNode::OwnedNode {
            place: place_iter.next().unwrap().get_mir_place().clone(),
            kind: node_permission_kind,
            children: Vec::new(),
        };
        let mut permission_kind = if node.get_place() == target_place || !target_type.is_write() {
            PermissionKind::ReadNode
        } else {
            PermissionKind::WriteNode
        };
        while let Some(component) = place_iter.next() {
            node = PermissionNode::OwnedNode {
                place: component.get_mir_place().clone(),
                kind: permission_kind,
                children: vec![node],
            };
            if component.get_mir_place() == target_place {
                permission_kind = PermissionKind::ReadNode;
            }
        }
        Self { root: node }
    }

    /// Add a new place by following the same rules as described in the
    /// comment for the `new`.
    pub fn add(
        &mut self,
        place: &mir::Place<'tcx>,
        _target_place: &mir::Place<'tcx>,
        target_type: TargetType,
    ) {
        let place = utils::VecPlace::new(place);
        let mut place_iter = place.iter();
        place_iter.next(); // Drop the root.
        debug!("place {:?}", place);
        debug!("self.root {:?}", self.root);
        let mut component_count = place.component_count() - 1; // Without root.
        if component_count == 0 {
            // Because we add all write nodes before read nodes, it can
            // happen that we have a tree already for which we want to add
            // a read node. In this case we just ignore the node being added.
            // TODO: Check if we do not need to make sure to add all of
            // its children.
            assert!(
                !target_type.is_write(),
                "Adding a write root node to an existing tree."
            );
            return;
        }
        let mut current_parent_node = &mut self.root;
        while component_count > 1 {
            let component = place_iter.next().unwrap();
            component_count -= 1;
            let current_node = current_parent_node
                .get_or_create_child(component.get_mir_place(), PermissionKind::ReadNode);
            if target_type.is_write() {
                current_node.set_permission_kind(PermissionKind::WriteNode);
            }
            current_parent_node = current_node;
        }
        let component = place_iter.next().unwrap();
        let kind = target_type.to_permission_kind();
        let mut _current_node =
            current_parent_node.get_or_create_child(component.get_mir_place(), kind);
    }

    pub fn get_root_place(&self) -> &mir::Place {
        self.root.get_place()
    }

    pub fn get_root(&self) -> &PermissionNode<'tcx> {
        &self.root
    }

    pub fn get_permissions(&self) -> Vec<(PermissionKind, mir::Place<'tcx>)> {
        trace!("[enter] get_permissions self={:?}", self);
        let mut visited = vec![];
        let mut to_visit = vec![&self.root];
        while let Some(node) = to_visit.pop() {
            let kind = node.get_permission_kind();
            for child in node.get_children().iter() {
                to_visit.push(child);
                if child.get_permission_kind() == PermissionKind::WriteNodeAndSubtree {
                    visited.push((PermissionKind::WriteNode, child.get_place().clone()));
                    continue;
                }
                match kind {
                    PermissionKind::ReadNode | PermissionKind::WriteNode => {
                        visited.push((kind, child.get_place().clone()));
                    }
                    _ => {
                        unreachable!();
                    }
                }
            }
            match kind {
                PermissionKind::ReadSubtree => {
                    visited.push((kind, node.get_place().clone()));
                }
                PermissionKind::WriteNodeAndSubtree | PermissionKind::WriteSubtree => {
                    visited.push((PermissionKind::WriteSubtree, node.get_place().clone()));
                }
                PermissionKind::ReadNode | PermissionKind::WriteNode | PermissionKind::None => {}
            }
        }
        trace!("[exit] get_permissions visited={:?}", visited);
        visited
    }
}

impl<'tcx> fmt::Display for PermissionTree<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.root)
    }
}

#[derive(Debug)]
pub struct PermissionForest<'tcx> {
    trees: Vec<PermissionTree<'tcx>>,
}

impl<'tcx> PermissionForest<'tcx> {
    /// +   `write_paths` – paths to whose leaves we should have write permission.
    /// +   `mut_borrowed_paths` – paths that are roots of trees to
    ///     which we hsould have write permission.
    /// +   `read_paths` – paths to whose leaves we should have read permission.
    pub fn new(
        write_paths: &Vec<mir::Place<'tcx>>,
        mut_borrowed_paths: &Vec<mir::Place<'tcx>>,
        read_paths: &Vec<mir::Place<'tcx>>,
        all_places: &PlaceSet<'tcx>,
    ) -> Self {
        trace!(
            "[enter] PermissionForest::new(\
             write_paths={:?}, \
             mut_borrowed_paths={:?}, \
             read_paths={:?}, \
             all_places={:?})",
            write_paths,
            mut_borrowed_paths,
            read_paths,
            all_places
        );

        let mut trees: Vec<PermissionTree> = Vec::new();

        /// Take the intended place to add and compute the set of places
        /// to add that are definitely initialised.
        fn compute_places_to_add<'a, 'tcx>(
            place: &'a mir::Place<'tcx>,
            all_places: &'a PlaceSet<'tcx>,
        ) -> Vec<(&'a mir::Place<'tcx>, &'a mir::Place<'tcx>)> {
            trace!(
                "[enter] compute_places_to_add(place={:?}, all_places={:?})",
                place,
                all_places
            );
            let mut found_def_init_prefix = false;
            let mut found_target_prefix = false;
            let mut result = Vec::new();
            for def_init_place in all_places.iter() {
                if utils::is_prefix(place, def_init_place) {
                    assert!(!found_target_prefix && !found_def_init_prefix);
                    result.push((place, place));
                    found_def_init_prefix = true;
                } else if utils::is_prefix(def_init_place, place) {
                    assert!(!found_def_init_prefix);
                    result.push((def_init_place, place));
                    found_target_prefix = true;
                }
            }
            assert!(found_target_prefix || found_def_init_prefix);
            result
        }

        /// Add places to the trees.
        fn add_paths<'tcx>(
            paths: &Vec<mir::Place<'tcx>>,
            trees: &mut Vec<PermissionTree<'tcx>>,
            target_type: TargetType,
            all_places: &PlaceSet<'tcx>,
        ) {
            for place in paths.iter() {
                let mut found = false;
                let places_to_add = compute_places_to_add(place, all_places);
                for tree in trees.iter_mut() {
                    if utils::is_prefix(place, tree.get_root_place()) {
                        found = true;
                        for (actual_place, target_place) in places_to_add.iter() {
                            tree.add(actual_place, target_place, target_type);
                        }
                    }
                }
                if !found {
                    for (actual_place, target_place) in places_to_add.iter() {
                        let tree = PermissionTree::new(actual_place, target_place, target_type);
                        trees.push(tree);
                    }
                }
            }
        }
        add_paths(write_paths, &mut trees, TargetType::WriteNode, all_places);
        add_paths(
            mut_borrowed_paths,
            &mut trees,
            TargetType::WriteContents,
            all_places,
        );
        add_paths(read_paths, &mut trees, TargetType::Read, all_places);
        Self { trees: trees }
    }

    pub fn get_trees(&self) -> &[PermissionTree<'tcx>] {
        &self.trees
    }
}

impl<'tcx> fmt::Display for PermissionForest<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut first = true;
        for tree in self.trees.iter() {
            if first {
                write!(f, "({})", tree.root)?;
                first = false;
            } else {
                write!(f, " && ({})", tree.root)?;
            }
        }
        Ok(())
    }
}
