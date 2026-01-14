//! Tree state model for hierarchical object browsing.
//!
//! Tracks expanded nodes, flattened view for rendering, and selection.

use std::collections::{HashMap, HashSet};

use crate::provider::ObjectInfo;

/// A node in the tree - either a directory or a file
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// The object info
    pub info: ObjectInfo,
    /// Depth in the tree (0 = root level)
    pub depth: usize,
    /// Parent key (empty for root items)
    pub parent_key: String,
    /// Whether this node has children (is a directory)
    pub is_dir: bool,
    /// Whether we've loaded children for this node
    pub children_loaded: bool,
    /// Number of children (if known)
    pub child_count: Option<usize>,
    /// Whether there are more children to load
    pub has_more_children: bool,
}

/// Tree state - manages expanded nodes and flattened view
#[derive(Debug, Default)]
pub struct TreeState {
    /// Set of expanded node keys
    pub expanded: HashSet<String>,
    /// All loaded nodes by their key
    pub nodes: HashMap<String, TreeNode>,
    /// Root-level node keys in order
    pub root_keys: Vec<String>,
    /// Flattened visible nodes (computed from expanded state)
    pub visible: Vec<String>,
    /// Currently selected index in visible list
    pub selected_index: usize,
    /// Currently loading prefixes
    pub loading: HashSet<String>,
}

impl TreeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set root-level items
    pub fn set_root(&mut self, objects: Vec<ObjectInfo>, has_more: bool) {
        self.root_keys.clear();

        for obj in objects {
            let key = obj.key.clone();
            let is_dir = obj.key.ends_with('/');

            let node = TreeNode {
                info: obj,
                depth: 0,
                parent_key: String::new(),
                is_dir,
                children_loaded: false,
                child_count: None,
                has_more_children: false,
            };

            self.nodes.insert(key.clone(), node);
            self.root_keys.push(key);
        }

        // Add a marker for "more items" if truncated
        if has_more {
            // We'll handle this in rendering
        }

        self.rebuild_visible();
    }

    /// Add children to an expanded node
    pub fn set_children(&mut self, parent_key: &str, objects: Vec<ObjectInfo>, has_more: bool) {
        let parent_depth = self.nodes.get(parent_key).map(|n| n.depth).unwrap_or(0);

        // Update parent node
        if let Some(parent) = self.nodes.get_mut(parent_key) {
            parent.children_loaded = true;
            parent.child_count = Some(objects.len());
            parent.has_more_children = has_more;
        }

        // Remove old children of this parent
        let old_children: Vec<String> = self.nodes
            .iter()
            .filter(|(_, n)| n.parent_key == parent_key)
            .map(|(k, _)| k.clone())
            .collect();

        for key in old_children {
            self.nodes.remove(&key);
        }

        // Add new children
        for obj in objects {
            let key = obj.key.clone();
            let is_dir = obj.key.ends_with('/');

            let node = TreeNode {
                info: obj,
                depth: parent_depth + 1,
                parent_key: parent_key.to_string(),
                is_dir,
                children_loaded: false,
                child_count: None,
                has_more_children: false,
            };

            self.nodes.insert(key, node);
        }

        self.rebuild_visible();
    }

    /// Toggle expanded state for a directory
    pub fn toggle_expanded(&mut self, key: &str) -> bool {
        if let Some(node) = self.nodes.get(key) {
            if !node.is_dir {
                return false;
            }

            if self.expanded.contains(key) {
                self.expanded.remove(key);
                self.rebuild_visible();
                false // collapsed
            } else {
                self.expanded.insert(key.to_string());
                self.rebuild_visible();
                true // expanded (may need to load children)
            }
        } else {
            false
        }
    }

    /// Check if a node is expanded
    pub fn is_expanded(&self, key: &str) -> bool {
        self.expanded.contains(key)
    }

    /// Check if a node needs to load children
    pub fn needs_children(&self, key: &str) -> bool {
        if let Some(node) = self.nodes.get(key) {
            node.is_dir && self.is_expanded(key) && !node.children_loaded
        } else {
            false
        }
    }

    /// Rebuild the flattened visible list based on expanded state
    pub fn rebuild_visible(&mut self) {
        let mut result = Vec::new();

        // Clone what we need to avoid borrow conflicts
        let root_keys = self.root_keys.clone();

        self.add_visible_recursive(&root_keys, "", &mut result);

        self.visible = result;

        // Clamp selection to valid range
        if !self.visible.is_empty() {
            self.selected_index = self.selected_index.min(self.visible.len() - 1);
        } else {
            self.selected_index = 0;
        }
    }

    /// Helper to recursively add visible nodes
    fn add_visible_recursive(&self, keys: &[String], parent_key: &str, result: &mut Vec<String>) {
        // Get children of parent
        let children: Vec<&String> = if parent_key.is_empty() {
            keys.iter().collect()
        } else {
            self.nodes
                .iter()
                .filter(|(_, n)| n.parent_key == parent_key)
                .map(|(k, _)| k)
                .collect()
        };

        // Sort by name (directories first, then alphabetically)
        let mut sorted: Vec<&String> = children;
        sorted.sort_by(|a, b| {
            let a_node = self.nodes.get(*a);
            let b_node = self.nodes.get(*b);
            match (a_node, b_node) {
                (Some(a), Some(b)) => {
                    // Directories first
                    match (a.is_dir, b.is_dir) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.info.name.cmp(&b.info.name),
                    }
                }
                _ => std::cmp::Ordering::Equal,
            }
        });

        for key in sorted {
            result.push(key.clone());

            // If expanded, add children recursively
            if self.expanded.contains(key) {
                self.add_visible_recursive(&[], key, result);
            }
        }
    }

    /// Get the currently selected node
    pub fn selected(&self) -> Option<&TreeNode> {
        self.visible
            .get(self.selected_index)
            .and_then(|key| self.nodes.get(key))
    }

    /// Get the currently selected key
    pub fn selected_key(&self) -> Option<&String> {
        self.visible.get(self.selected_index)
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if !self.visible.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.visible.len() - 1);
        }
    }

    /// Move to first item
    pub fn select_first(&mut self) {
        self.selected_index = 0;
    }

    /// Move to last item
    pub fn select_last(&mut self) {
        if !self.visible.is_empty() {
            self.selected_index = self.visible.len() - 1;
        }
    }

    /// Set loading state for a prefix
    pub fn set_loading(&mut self, key: &str, loading: bool) {
        if loading {
            self.loading.insert(key.to_string());
        } else {
            self.loading.remove(key);
        }
    }

    /// Check if a prefix is loading
    pub fn is_loading(&self, key: &str) -> bool {
        self.loading.contains(key)
    }

    /// Check if anything is loading
    pub fn any_loading(&self) -> bool {
        !self.loading.is_empty()
    }

    /// Get visible nodes for rendering
    pub fn visible_nodes(&self) -> Vec<(&String, &TreeNode)> {
        self.visible
            .iter()
            .filter_map(|key| self.nodes.get(key).map(|node| (key, node)))
            .collect()
    }

    /// Get position info for a node (is it last child of parent?)
    pub fn is_last_in_parent(&self, key: &str) -> bool {
        if let Some(node) = self.nodes.get(key) {
            let siblings: Vec<&String> = if node.parent_key.is_empty() {
                self.root_keys.iter().collect()
            } else {
                self.nodes
                    .iter()
                    .filter(|(_, n)| n.parent_key == node.parent_key)
                    .map(|(k, _)| k)
                    .collect()
            };

            siblings.last().map(|k| *k == key).unwrap_or(false)
        } else {
            false
        }
    }

    /// Get the tree prefix characters for a node
    pub fn get_tree_prefix(&self, key: &str) -> String {
        let mut prefix = String::new();

        if let Some(node) = self.nodes.get(key) {
            // Build prefix from ancestors
            let mut ancestors: Vec<String> = Vec::new();
            let mut current_parent = node.parent_key.clone();

            while !current_parent.is_empty() {
                ancestors.push(current_parent.clone());
                if let Some(parent_node) = self.nodes.get(&current_parent) {
                    current_parent = parent_node.parent_key.clone();
                } else {
                    break;
                }
            }

            ancestors.reverse();

            // For each ancestor level, add │ or space
            for ancestor_key in &ancestors {
                if self.is_last_in_parent(ancestor_key) {
                    prefix.push_str("   ");
                } else {
                    prefix.push_str("│  ");
                }
            }

            // Add the connector for this node
            if node.depth > 0 {
                if self.is_last_in_parent(key) {
                    prefix.push_str("└─ ");
                } else {
                    prefix.push_str("├─ ");
                }
            }
        }

        prefix
    }
}
