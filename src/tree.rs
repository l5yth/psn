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

use std::collections::{HashMap, HashSet};

use crate::{model::ProcRow, process::compare_rows};

const INIT_PID: i32 = 1;

/// A single visible row in the rendered process tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayRow {
    /// Backing index into the app row list.
    pub row_index: usize,
    /// Connector prefix shown before the process name.
    pub prefix: String,
    /// Whether the row has visible descendants in the current tree graph.
    pub has_children: bool,
    /// Whether the row's descendants are currently hidden.
    pub is_collapsed: bool,
}

/// Build visible tree rows with connector prefixes and collapse metadata.
pub fn display_rows(rows: &[ProcRow], collapsed_pids: &HashSet<i32>) -> Vec<DisplayRow> {
    let tree = TreeLayout::build(rows);
    walk_display_rows(rows, &tree, collapsed_pids)
}

/// Build display order with tree connector prefixes.
pub fn display_order_with_prefix(
    rows: &[ProcRow],
    collapsed_pids: &HashSet<i32>,
) -> Vec<(usize, String)> {
    display_rows(rows, collapsed_pids)
        .into_iter()
        .map(|row| (row.row_index, row.prefix))
        .collect()
}

/// Build display order as backing-row indices only.
pub fn display_order_indices(rows: &[ProcRow], collapsed_pids: &HashSet<i32>) -> Vec<usize> {
    display_rows(rows, collapsed_pids)
        .into_iter()
        .map(|row| row.row_index)
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
    context: &mut WalkContext<'_>,
    idx: usize,
    ancestor_has_next: &[bool],
    is_last: bool,
    is_root: bool,
) {
    if !context.visited.insert(idx) {
        return;
    }

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

    let has_children = context
        .children
        .get(&context.rows[idx].pid)
        .is_some_and(|next| !next.is_empty());
    let is_collapsed = has_children && context.collapsed_pids.contains(&context.rows[idx].pid);

    context.ordered.push(DisplayRow {
        row_index: idx,
        prefix,
        has_children,
        is_collapsed,
    });

    if is_collapsed {
        mark_descendants_hidden(context, idx);
        return;
    }

    if let Some(next) = context.children.get(&context.rows[idx].pid) {
        for (child_pos, child) in next.iter().enumerate() {
            let child_is_last = child_pos + 1 == next.len();
            let mut next_ancestors = ancestor_has_next.to_vec();
            if !is_root {
                next_ancestors.push(!is_last);
            }
            walk_tree(context, *child, &next_ancestors, child_is_last, false);
        }
    }
}

fn sort_indices(indices: &mut [usize], rows: &[ProcRow]) {
    indices.sort_by(|left, right| compare_rows(&rows[*left], &rows[*right]));
}

struct WalkContext<'a> {
    rows: &'a [ProcRow],
    children: &'a HashMap<i32, Vec<usize>>,
    collapsed_pids: &'a HashSet<i32>,
    ordered: &'a mut Vec<DisplayRow>,
    visited: &'a mut HashSet<usize>,
}

struct TreeLayout {
    roots: Vec<usize>,
    children: HashMap<i32, Vec<usize>>,
}

impl TreeLayout {
    fn build(rows: &[ProcRow]) -> Self {
        let mut pid_to_index: HashMap<i32, usize> = HashMap::new();
        for (idx, row) in rows.iter().enumerate() {
            pid_to_index.insert(row.pid, idx);
        }

        let mut roots: Vec<usize> = Vec::new();
        let mut children: HashMap<i32, Vec<usize>> = HashMap::new();
        for (idx, row) in rows.iter().enumerate() {
            if row.ppid == Some(INIT_PID) {
                roots.push(idx);
                continue;
            }
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

        Self { roots, children }
    }
}

fn walk_display_rows(
    rows: &[ProcRow],
    tree: &TreeLayout,
    collapsed_pids: &HashSet<i32>,
) -> Vec<DisplayRow> {
    let mut ordered: Vec<DisplayRow> = Vec::with_capacity(rows.len());
    let mut visited: HashSet<usize> = HashSet::with_capacity(rows.len());
    let mut context = WalkContext {
        rows,
        children: &tree.children,
        collapsed_pids,
        ordered: &mut ordered,
        visited: &mut visited,
    };
    for (root_pos, root) in tree.roots.iter().enumerate() {
        let is_last_root = root_pos + 1 == tree.roots.len();
        walk_tree(&mut context, *root, &[], is_last_root, true);
    }

    if context.visited.len() < rows.len() {
        let mut remaining: Vec<usize> = (0..rows.len())
            .filter(|idx| !context.visited.contains(idx))
            .collect();
        sort_indices(&mut remaining, rows);
        for idx in remaining {
            walk_tree(&mut context, idx, &[], true, true);
        }
    }

    ordered
}

fn mark_descendants_hidden(context: &mut WalkContext<'_>, idx: usize) {
    let Some(children) = context.children.get(&context.rows[idx].pid) else {
        return;
    };

    let child_indices = children.clone();
    for child in child_indices {
        if context.visited.insert(child) {
            mark_descendants_hidden(context, child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{display_order_indices, display_order_with_prefix, display_rows};
    use crate::model::ProcRow;
    use std::{collections::HashSet, sync::Arc};
    use sysinfo::ProcessStatus;

    fn row(pid: i32, ppid: Option<i32>, ancestors: Vec<i32>, name: &str) -> ProcRow {
        ProcRow {
            pid,
            start_time: 0,
            ppid,
            ancestor_chain: ancestors,
            user: Arc::from("user"),
            status: ProcessStatus::Sleep,
            cpu_usage_tenths: 0,
            memory_bytes: 0,
            name: name.to_string(),
            cmd: name.to_string(),
        }
    }

    #[test]
    fn display_order_indices_keeps_rows_when_parent_graph_is_cycle_only() {
        let rows = vec![
            row(2, Some(3), vec![3, 2], "a"),
            row(3, Some(2), vec![2, 3], "b"),
        ];

        let order = display_order_indices(&rows, &HashSet::new());
        assert_eq!(order.len(), 2);
        assert!(order.contains(&0));
        assert!(order.contains(&1));
    }

    #[test]
    fn display_order_with_prefix_treats_pid_one_children_as_roots() {
        let rows = vec![
            row(1, None, Vec::new(), "init"),
            row(2, Some(1), vec![1], "service"),
            row(3, Some(2), vec![2, 1], "worker"),
        ];

        let order = display_order_with_prefix(&rows, &HashSet::new());
        assert_eq!(
            order,
            vec![
                (0, "".to_string()),
                (1, "".to_string()),
                (2, "└─".to_string())
            ]
        );
    }

    #[test]
    fn display_rows_hide_collapsed_descendants_and_mark_root() {
        let rows = vec![
            row(1, None, Vec::new(), "init"),
            row(2, Some(1), vec![1], "service"),
            row(3, Some(2), vec![2, 1], "worker"),
        ];
        let collapsed = HashSet::from([2]);

        let order = display_rows(&rows, &collapsed);
        assert_eq!(order.len(), 2);
        assert_eq!(order[1].row_index, 1);
        assert!(order[1].has_children);
        assert!(order[1].is_collapsed);
    }
}
