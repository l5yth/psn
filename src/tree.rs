/*
   Copyright (C) 2026 l5yth

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
*/

//! Tree ordering helpers shared by rendering and row-selection logic.

use std::collections::HashMap;

use crate::{model::ProcRow, process::status_priority};

/// Build display order with tree connector prefixes.
pub fn display_order_with_prefix(rows: &[ProcRow]) -> Vec<(usize, String)> {
    let mut pid_to_index: HashMap<i32, usize> = HashMap::new();
    for (idx, row) in rows.iter().enumerate() {
        pid_to_index.insert(row.pid, idx);
    }

    let mut roots: Vec<usize> = Vec::new();
    let mut children: HashMap<i32, Vec<usize>> = HashMap::new();
    for (idx, row) in rows.iter().enumerate() {
        if let Some(parent_pid) = nearest_visible_ancestor(&row.ancestor_chain, &pid_to_index) {
            children.entry(parent_pid).or_default().push(idx);
            continue;
        }
        roots.push(idx);
    }

    sort_indices(&mut roots, rows);
    for child_group in children.values_mut() {
        sort_indices(child_group, rows);
    }

    let mut ordered: Vec<(usize, String)> = Vec::with_capacity(rows.len());
    for (root_pos, root) in roots.iter().enumerate() {
        let is_last_root = root_pos + 1 == roots.len();
        walk_tree(
            *root,
            rows,
            &children,
            &mut ordered,
            &[],
            is_last_root,
            true,
        );
    }
    ordered
}

/// Build display order as backing-row indices only.
pub fn display_order_indices(rows: &[ProcRow]) -> Vec<usize> {
    display_order_with_prefix(rows)
        .into_iter()
        .map(|(idx, _)| idx)
        .collect()
}

fn nearest_visible_ancestor(ancestor_chain: &[i32], visible: &HashMap<i32, usize>) -> Option<i32> {
    for candidate in ancestor_chain {
        if visible.contains_key(candidate) {
            return Some(*candidate);
        }
    }
    None
}

fn walk_tree(
    idx: usize,
    rows: &[ProcRow],
    children: &HashMap<i32, Vec<usize>>,
    ordered: &mut Vec<(usize, String)>,
    ancestor_has_next: &[bool],
    is_last: bool,
    is_root: bool,
) {
    let mut prefix = String::new();
    for has_next in ancestor_has_next {
        if *has_next {
            prefix.push_str("│ ");
        } else {
            prefix.push_str("  ");
        }
    }

    if !is_root {
        if is_last {
            prefix.push_str("└─");
        } else {
            prefix.push_str("├─");
        }
    }

    ordered.push((idx, prefix));
    if let Some(next) = children.get(&rows[idx].pid) {
        for (child_pos, child) in next.iter().enumerate() {
            let child_is_last = child_pos + 1 == next.len();
            let mut next_ancestors = ancestor_has_next.to_vec();
            if !is_root {
                next_ancestors.push(!is_last);
            }
            walk_tree(
                *child,
                rows,
                children,
                ordered,
                &next_ancestors,
                child_is_last,
                false,
            );
        }
    }
}

fn sort_indices(indices: &mut [usize], rows: &[ProcRow]) {
    indices.sort_by(|left, right| {
        status_priority(rows[*left].status)
            .cmp(&status_priority(rows[*right].status))
            .then(rows[*left].pid.cmp(&rows[*right].pid))
            .then(rows[*left].name.cmp(&rows[*right].name))
            .then(rows[*left].user.as_ref().cmp(rows[*right].user.as_ref()))
            .then(rows[*left].cmd.cmp(&rows[*right].cmd))
    });
}
