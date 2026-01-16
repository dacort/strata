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
    /// Continuation token for loading more children
    pub continuation_token: Option<String>,
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
            // Treat directories and ZIP archives as expandable
            let is_dir = obj.key.ends_with('/')
                || (obj.name.to_lowercase().ends_with(".zip") && !obj.key.contains('#'));

            let node = TreeNode {
                info: obj,
                depth: 0,
                parent_key: String::new(),
                is_dir,
                children_loaded: false,
                child_count: None,
                has_more_children: false,
                continuation_token: None,
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
    pub fn set_children(
        &mut self,
        parent_key: &str,
        objects: Vec<ObjectInfo>,
        has_more: bool,
        continuation_token: Option<String>,
    ) {
        eprintln!("  [set_children] parent_key='{}', objects={}", parent_key, objects.len());
        let parent_depth = self.nodes.get(parent_key).map(|n| n.depth).unwrap_or(0);
        eprintln!("  [set_children] parent_depth={}", parent_depth);

        // Update parent node
        if let Some(parent) = self.nodes.get_mut(parent_key) {
            parent.children_loaded = true;
            parent.child_count = Some(objects.len());
            parent.has_more_children = has_more;
            parent.continuation_token = continuation_token;
        }

        // Identify expanded children that should be preserved
        // These are children of parent_key that are currently expanded
        let expanded_children: HashSet<String> = self
            .nodes
            .iter()
            .filter(|(k, n)| {
                n.parent_key == parent_key && *k != parent_key && self.expanded.contains(*k)
            })
            .map(|(k, _)| k.clone())
            .collect();

        eprintln!("  [set_children] expanded_children: {:?}", expanded_children);

        // Collect all descendants of expanded children (entire subtrees to preserve)
        let mut nodes_to_preserve: HashSet<String> = expanded_children.clone();
        for expanded_key in &expanded_children {
            self.collect_all_descendants(expanded_key, &mut nodes_to_preserve);
        }

        eprintln!("  [set_children] nodes_to_preserve: {:?}", nodes_to_preserve);

        // Remove old children that are NOT in the preserve set
        let old_children: Vec<String> = self
            .nodes
            .iter()
            .filter(|(k, n)| {
                n.parent_key == parent_key && *k != parent_key && !nodes_to_preserve.contains(*k)
            })
            .map(|(k, _)| k.clone())
            .collect();

        eprintln!("  [set_children] will remove {} old children: {:?}", old_children.len(), old_children);

        for key in old_children {
            self.nodes.remove(&key);
        }

        // Add or update children from the new data
        for obj in objects {
            let key = obj.key.clone();

            // Skip if this is an expanded node we're preserving
            if nodes_to_preserve.contains(&key) {
                continue;
            }

            // Check if it's a directory (ends with /) or a ZIP archive (can be expanded)
            let is_dir = obj.key.ends_with('/')
                || (obj.name.to_lowercase().ends_with(".zip") && !obj.key.contains('#'));

            let node = TreeNode {
                info: obj,
                depth: parent_depth + 1,
                parent_key: parent_key.to_string(),
                is_dir,
                children_loaded: false,
                child_count: None,
                has_more_children: false,
                continuation_token: None,
            };

            self.nodes.insert(key, node);
        }

        // Debug: Log ZIP-related nodes after modification
        let zip_nodes: Vec<_> = self.nodes.keys().filter(|k| k.contains(".zip")).collect();
        eprintln!("  [set_children] ZIP-related nodes after: {:?}", zip_nodes);

        self.rebuild_visible();
    }

    /// Helper to collect all descendants of a node recursively
    fn collect_all_descendants(&self, key: &str, result: &mut HashSet<String>) {
        for (child_key, node) in &self.nodes {
            if node.parent_key == key {
                result.insert(child_key.clone());
                self.collect_all_descendants(child_key, result);
            }
        }
    }

    /// Append more children to an already expanded node (for pagination)
    pub fn append_children(
        &mut self,
        parent_key: &str,
        objects: Vec<ObjectInfo>,
        has_more: bool,
        continuation_token: Option<String>,
    ) {
        let parent_depth = self.nodes.get(parent_key).map(|n| n.depth).unwrap_or(0);

        // Update parent node
        if let Some(parent) = self.nodes.get_mut(parent_key) {
            let current_count = parent.child_count.unwrap_or(0);
            parent.child_count = Some(current_count + objects.len());
            parent.has_more_children = has_more;
            parent.continuation_token = continuation_token;
        }

        // Add new children (don't remove existing ones)
        for obj in objects {
            let key = obj.key.clone();
            // Check if it's a directory (ends with /) or a ZIP archive (can be expanded)
            let is_dir = obj.key.ends_with('/')
                || (obj.name.to_lowercase().ends_with(".zip") && !obj.key.contains('#'));

            let node = TreeNode {
                info: obj,
                depth: parent_depth + 1,
                parent_key: parent_key.to_string(),
                is_dir,
                children_loaded: false,
                child_count: None,
                has_more_children: false,
                continuation_token: None,
            };

            self.nodes.insert(key, node);
        }

        self.rebuild_visible();
    }

    /// Get the continuation token for a node
    pub fn get_continuation_token(&self, key: &str) -> Option<String> {
        self.nodes
            .get(key)
            .and_then(|n| n.continuation_token.clone())
    }

    /// Toggle expanded state for a directory or archive
    pub fn toggle_expanded(&mut self, key: &str) -> bool {
        if let Some(node) = self.nodes.get(key) {
            // Allow expansion for directories OR ZIP archives
            let is_expandable = node.is_dir || node.info.name.to_lowercase().ends_with(".zip");

            if !is_expandable {
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
            let is_expandable = node.is_dir || node.info.name.to_lowercase().ends_with(".zip");
            is_expandable && self.is_expanded(key) && !node.children_loaded
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

    /// Navigate to next sibling directory (skip over children and files)
    /// If no sibling at current level, go to parent's next sibling, etc.
    /// Returns true if navigation happened, false if no next directory found
    pub fn select_next_sibling(&mut self) -> bool {
        let Some(current_key) = self.selected_key().cloned() else {
            return false;
        };
        let Some(current_node) = self.nodes.get(&current_key) else {
            return false;
        };

        let current_depth = current_node.depth;

        // Look for the next directory at the same depth or shallower
        // This naturally handles: next sibling, or parent's next sibling, etc.
        for (idx, key) in self
            .visible
            .iter()
            .enumerate()
            .skip(self.selected_index + 1)
        {
            if let Some(node) = self.nodes.get(key) {
                // Found a directory at same level or higher (shallower) = that's our target
                if node.is_dir && node.depth <= current_depth {
                    self.selected_index = idx;
                    return true;
                }
            }
        }
        false
    }

    /// Navigate to previous sibling directory
    /// If no sibling at current level, go to parent directory
    /// Returns true if navigation happened, false if no previous directory found
    pub fn select_prev_sibling(&mut self) -> bool {
        let Some(current_key) = self.selected_key().cloned() else {
            return false;
        };
        let Some(current_node) = self.nodes.get(&current_key) else {
            return false;
        };

        let current_depth = current_node.depth;

        // Look backwards for a directory at the same depth or shallower
        for idx in (0..self.selected_index).rev() {
            if let Some(key) = self.visible.get(idx) {
                if let Some(node) = self.nodes.get(key) {
                    // Found a directory at same level or higher = that's our target
                    if node.is_dir && node.depth <= current_depth {
                        self.selected_index = idx;
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if the current selection is at the "load more" boundary.
    /// Returns Some(parent_key) if we're at the last loaded item of a parent that has more children.
    pub fn at_load_more_boundary(&self) -> Option<String> {
        let current_key = self.selected_key()?;
        let current_node = self.nodes.get(current_key)?;
        let parent_key = &current_node.parent_key;

        if parent_key.is_empty() {
            // At root level - TODO: handle root pagination later
            return None;
        }

        let parent_node = self.nodes.get(parent_key)?;

        // Check if parent has more children and we're at the last visible descendant
        if !parent_node.has_more_children || parent_node.continuation_token.is_none() {
            return None;
        }

        // Check if next visible node is NOT a descendant of our parent
        let next_idx = self.selected_index + 1;
        if next_idx < self.visible.len() {
            let next_key = &self.visible[next_idx];
            // If next node starts with parent_key, it's still a descendant
            if next_key.starts_with(parent_key) {
                return None;
            }
        }

        // We're at the boundary!
        Some(parent_key.clone())
    }

    /// Cancel loading for a specific prefix
    pub fn cancel_loading(&mut self, key: &str) {
        self.loading.remove(key);
    }

    /// Cancel all loading operations
    pub fn cancel_all_loading(&mut self) {
        self.loading.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_continuation_token_storage() {
        let mut tree = TreeState::new();

        // Add a root directory
        let root_obj = ObjectInfo::prefix("data/", "data/");
        tree.set_root(vec![root_obj], false);

        // Add children with continuation token
        let children = vec![
            ObjectInfo::object("file1.txt", "data/file1.txt", 100),
            ObjectInfo::object("file2.txt", "data/file2.txt", 200),
        ];
        let token = Some("next-page-token".to_string());
        tree.set_children("data/", children, true, token.clone());

        // Verify the parent has the continuation token
        let parent = tree.nodes.get("data/").unwrap();
        assert_eq!(parent.has_more_children, true);
        assert_eq!(parent.continuation_token, token);
        assert_eq!(parent.child_count, Some(2));
    }

    #[test]
    fn test_append_children() {
        let mut tree = TreeState::new();

        // Setup: add root and initial children
        tree.set_root(vec![ObjectInfo::prefix("logs/", "logs/")], false);
        let initial_children = vec![ObjectInfo::object("app.log", "logs/app.log", 100)];
        tree.set_children("logs/", initial_children, true, Some("token1".to_string()));

        // Verify initial state
        assert_eq!(tree.nodes.get("logs/").unwrap().child_count, Some(1));
        assert_eq!(tree.nodes.get("logs/").unwrap().has_more_children, true);

        // Append more children
        let more_children = vec![
            ObjectInfo::object("error.log", "logs/error.log", 200),
            ObjectInfo::object("debug.log", "logs/debug.log", 300),
        ];
        tree.append_children("logs/", more_children, false, None);

        // Verify appended state
        let parent = tree.nodes.get("logs/").unwrap();
        assert_eq!(parent.child_count, Some(3)); // 1 + 2 = 3
        assert_eq!(parent.has_more_children, false);
        assert_eq!(parent.continuation_token, None);

        // Verify all children exist
        assert!(tree.nodes.contains_key("logs/app.log"));
        assert!(tree.nodes.contains_key("logs/error.log"));
        assert!(tree.nodes.contains_key("logs/debug.log"));
    }

    #[test]
    fn test_get_continuation_token() {
        let mut tree = TreeState::new();

        tree.set_root(vec![ObjectInfo::prefix("data/", "data/")], false);
        tree.set_children(
            "data/",
            vec![ObjectInfo::object("file.txt", "data/file.txt", 100)],
            true,
            Some("my-token".to_string()),
        );

        // Should return the token
        assert_eq!(
            tree.get_continuation_token("data/"),
            Some("my-token".to_string())
        );

        // Non-existent key should return None
        assert_eq!(tree.get_continuation_token("nonexistent/"), None);
    }

    #[test]
    fn test_zip_file_stays_visible_when_expanded() {
        let mut tree = TreeState::new();

        // Add a ZIP file at root level
        let zip_obj = ObjectInfo::object("archive.zip", "archive.zip", 1024);
        tree.set_root(vec![zip_obj], false);

        // Verify ZIP is in root and visible
        assert!(tree.nodes.contains_key("archive.zip"));
        assert_eq!(tree.visible.len(), 1);
        assert_eq!(tree.visible[0], "archive.zip");

        // Expand the ZIP file
        tree.toggle_expanded("archive.zip");

        // Add children (simulating ZIP contents)
        let children = vec![
            ObjectInfo::object("file1.txt", "archive.zip#file1.txt", 100),
            ObjectInfo::object("file2.txt", "archive.zip#file2.txt", 200),
        ];
        tree.set_children("archive.zip", children, false, None);

        // CRITICAL: ZIP file itself should still exist in nodes
        assert!(
            tree.nodes.contains_key("archive.zip"),
            "ZIP file should not be removed from nodes"
        );

        // CRITICAL: ZIP file should still be visible
        assert!(
            tree.visible.contains(&"archive.zip".to_string()),
            "ZIP file should remain visible after expansion"
        );

        // Children should also be visible
        assert!(tree.visible.contains(&"archive.zip#file1.txt".to_string()));
        assert!(tree.visible.contains(&"archive.zip#file2.txt".to_string()));

        // Verify ordering: ZIP file should come before its children
        let zip_idx = tree
            .visible
            .iter()
            .position(|k| k == "archive.zip")
            .unwrap();
        let child1_idx = tree
            .visible
            .iter()
            .position(|k| k == "archive.zip#file1.txt")
            .unwrap();
        assert!(
            zip_idx < child1_idx,
            "ZIP file should appear before its children"
        );
    }

    #[test]
    fn test_nested_zip_file_stays_visible() {
        let mut tree = TreeState::new();

        // Add a directory with a ZIP file inside
        let dir_obj = ObjectInfo::prefix("data/", "data/");
        tree.set_root(vec![dir_obj], false);

        // Expand directory and add ZIP file
        tree.toggle_expanded("data/");
        let zip_obj = ObjectInfo::object("archive.zip", "data/archive.zip", 1024);
        tree.set_children("data/", vec![zip_obj], false, None);

        // Verify ZIP file exists
        assert!(tree.nodes.contains_key("data/archive.zip"));

        // Expand the ZIP file
        tree.toggle_expanded("data/archive.zip");

        // Add ZIP contents
        let children = vec![ObjectInfo::object(
            "nested.txt",
            "data/archive.zip#nested.txt",
            50,
        )];
        tree.set_children("data/archive.zip", children, false, None);

        // CRITICAL: ZIP file should still exist
        assert!(
            tree.nodes.contains_key("data/archive.zip"),
            "Nested ZIP file should not be removed"
        );

        // CRITICAL: ZIP file should be in visible list
        assert!(
            tree.visible.contains(&"data/archive.zip".to_string()),
            "Nested ZIP file should remain visible"
        );

        // Child should also be visible
        assert!(
            tree.visible
                .contains(&"data/archive.zip#nested.txt".to_string())
        );
    }

    #[test]
    fn test_parent_reload_preserves_expanded_zip() {
        let mut tree = TreeState::new();

        // Setup: Create a directory with a ZIP file inside
        let dir_obj = ObjectInfo::prefix("data/", "data/");
        tree.set_root(vec![dir_obj], false);

        // Expand the directory and add children including a ZIP
        tree.toggle_expanded("data/");
        let children = vec![
            ObjectInfo::object("file.txt", "data/file.txt", 100),
            ObjectInfo::object("archive.zip", "data/archive.zip", 1024),
        ];
        tree.set_children("data/", children, false, None);

        // Verify initial state
        assert!(tree.nodes.contains_key("data/archive.zip"));
        assert_eq!(tree.nodes.len(), 3); // data/, file.txt, archive.zip

        // Expand the ZIP file
        tree.toggle_expanded("data/archive.zip");
        let zip_children = vec![
            ObjectInfo::object("internal1.txt", "data/archive.zip#internal1.txt", 50),
            ObjectInfo::object("internal2.txt", "data/archive.zip#internal2.txt", 75),
        ];
        tree.set_children("data/archive.zip", zip_children, false, None);

        // Verify ZIP is expanded with children
        assert!(tree.nodes.contains_key("data/archive.zip"));
        assert!(tree.nodes.contains_key("data/archive.zip#internal1.txt"));
        assert!(tree.nodes.contains_key("data/archive.zip#internal2.txt"));
        assert_eq!(tree.nodes.len(), 5); // data/, file.txt, archive.zip, internal1.txt, internal2.txt

        // CRITICAL TEST: Simulate parent directory reload (the bug scenario)
        // When the parent directory reloads, it should preserve the expanded ZIP and its children
        let new_children = vec![
            ObjectInfo::object("file.txt", "data/file.txt", 100),
            ObjectInfo::object("archive.zip", "data/archive.zip", 1024),
            ObjectInfo::object("newfile.txt", "data/newfile.txt", 200), // New file added
        ];
        tree.set_children("data/", new_children, false, None);

        // VERIFY: ZIP file and its expanded children should still exist
        assert!(
            tree.nodes.contains_key("data/archive.zip"),
            "ZIP file should be preserved"
        );
        assert!(
            tree.nodes.contains_key("data/archive.zip#internal1.txt"),
            "ZIP child 1 should be preserved"
        );
        assert!(
            tree.nodes.contains_key("data/archive.zip#internal2.txt"),
            "ZIP child 2 should be preserved"
        );
        assert!(
            tree.nodes.contains_key("data/newfile.txt"),
            "New file should be added"
        );

        // VERIFY: ZIP should still be marked as expanded
        assert!(
            tree.is_expanded("data/archive.zip"),
            "ZIP should still be expanded"
        );

        // VERIFY: ZIP children should be visible
        assert!(
            tree.visible
                .contains(&"data/archive.zip#internal1.txt".to_string()),
            "ZIP child 1 should be visible"
        );
    }
}
