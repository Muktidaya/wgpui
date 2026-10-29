//! Semantic accessibility properties and per-frame AccessKit tree construction.

use crate::{GlobalElementId, SharedString};
use accesskit::{Node, NodeId, TreeId, TreeInfo, TreeUpdate};
use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

/// Author-facing ARIA-like properties stored on interactive elements.
#[derive(Clone, Debug, Default)]
#[allow(missing_docs)]
pub struct AriaProperties {
    pub author_id: Option<SharedString>,
    pub label: Option<SharedString>,
    pub description: Option<SharedString>,
    pub keyshortcuts: Option<SharedString>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    pub toggled: Option<accesskit::Toggled>,
    pub numeric_value: Option<f64>,
    pub numeric_value_step: Option<f64>,
    pub value: Option<SharedString>,
    pub placeholder: Option<SharedString>,
    pub min_numeric_value: Option<f64>,
    pub max_numeric_value: Option<f64>,
    pub orientation: Option<accesskit::Orientation>,
    pub level: Option<usize>,
    pub position_in_set: Option<usize>,
    pub size_of_set: Option<usize>,
    pub row_index: Option<usize>,
    pub column_index: Option<usize>,
    pub row_count: Option<usize>,
    pub column_count: Option<usize>,
}

pub(crate) const ROOT_NODE_ID: NodeId = NodeId(0);
const SYNTHETIC_NAMESPACE: u64 = 1 << 63;

fn ordinary_node_id(key: impl Hash) -> NodeId {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    NodeId((hasher.finish() & !SYNTHETIC_NAMESPACE).max(1))
}

pub(crate) fn node_id_for_element(id: &GlobalElementId) -> NodeId {
    ordinary_node_id(("element", id))
}

pub(crate) fn node_id_for_ordinal(ordinal: usize) -> NodeId {
    ordinary_node_id(("ordinal", ordinal))
}

/// Adds synthetic accessibility children to the element currently being visited.
///
/// Child identifiers remain stable when the same parent and key are used in
/// subsequent frames. Their parent relationship also lets actions on synthetic
/// text nodes reach the element that owns the text.
pub struct A11ySubtreeBuilder<'a> {
    parent_id: NodeId,
    parent: &'a mut Node,
    nodes: &'a mut Vec<(NodeId, Node)>,
    seen_ids: &'a mut HashSet<NodeId>,
    parents: &'a mut HashMap<NodeId, NodeId>,
}

impl<'a> A11ySubtreeBuilder<'a> {
    pub(crate) fn new(parent_id: NodeId, nodes: &'a mut A11yNodeBuilder) -> Option<Self> {
        let (current_id, parent) = nodes.stack.last_mut()?;
        if *current_id != parent_id {
            return None;
        }
        Some(Self {
            parent_id,
            parent,
            nodes: &mut nodes.nodes,
            seen_ids: &mut nodes.seen_ids,
            parents: &mut nodes.parents,
        })
    }

    /// Derives a stable identifier from a key unique within this parent.
    ///
    /// Synthetic identifiers occupy a separate namespace from ordinary element
    /// identifiers and the window root.
    pub fn synthetic_node_id(&self, key: impl Hash) -> NodeId {
        let mut hasher = DefaultHasher::new();
        self.parent_id.hash(&mut hasher);
        key.hash(&mut hasher);
        NodeId(hasher.finish() | SYNTHETIC_NAMESPACE)
    }

    /// Appends a child and its semantic metadata, returning false for duplicate IDs.
    pub fn push_child(&mut self, id: NodeId, node: Node) -> bool {
        if !self.seen_ids.insert(id) {
            log::warn!("Duplicate accessibility node {id:?} was rejected");
            return false;
        }
        if !self.parent.children().contains(&id) {
            self.parent.push_child(id);
        }
        self.parents.insert(id, self.parent_id);
        self.nodes.push((id, node));
        true
    }

    /// Returns the actual parent node so its metadata can be refined.
    pub fn parent_node(&mut self) -> &mut Node {
        self.parent
    }
}

#[derive(Default)]
pub(crate) struct A11yNodeBuilder {
    stack: Vec<(NodeId, Node)>,
    nodes: Vec<(NodeId, Node)>,
    seen_ids: HashSet<NodeId>,
    parents: HashMap<NodeId, NodeId>,
    focus: Option<NodeId>,
    active_descendant: Option<NodeId>,
}

impl A11yNodeBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn begin_frame(&mut self, window_title: Option<&SharedString>) {
        self.stack.clear();
        self.nodes.clear();
        self.seen_ids.clear();
        self.parents.clear();
        self.focus = None;
        self.active_descendant = None;
        let mut root = Node::new(accesskit::Role::Window);
        if let Some(title) = window_title {
            root.set_label(title.to_string());
        }
        self.stack.push((ROOT_NODE_ID, root));
        self.seen_ids.insert(ROOT_NODE_ID);
    }

    pub(crate) fn push(&mut self, id: NodeId, node: Node) -> bool {
        if !self.attach(id) {
            return false;
        }
        self.stack.push((id, node));
        true
    }

    #[cfg(test)]
    pub(crate) fn push_leaf(&mut self, id: NodeId, node: Node) -> bool {
        if !self.attach(id) {
            return false;
        }
        self.nodes.push((id, node));
        true
    }

    fn attach(&mut self, id: NodeId) -> bool {
        let Some((parent_id, parent)) = self.stack.last_mut() else {
            log::error!("Accessibility node {id:?} was submitted outside a frame");
            return false;
        };
        if !self.seen_ids.insert(id) {
            log::warn!("Duplicate accessibility node {id:?} was rejected");
            return false;
        }
        if !parent.children().contains(&id) {
            parent.push_child(id);
        }
        self.parents.insert(id, *parent_id);
        true
    }

    pub(crate) fn pop(&mut self) {
        if self.stack.len() <= 1 {
            log::error!("Accessibility traversal attempted to pop the window root");
            return;
        }
        if let Some(node) = self.stack.pop() {
            self.nodes.push(node);
        }
    }

    #[cfg(test)]
    pub(crate) fn current_node_mut(&mut self) -> Option<&mut Node> {
        self.stack.last_mut().map(|(_, node)| node)
    }

    pub(crate) fn has_node(&self, id: NodeId) -> bool {
        self.seen_ids.contains(&id)
    }

    pub(crate) fn node_by_id(&self, id: NodeId) -> Option<&Node> {
        self.stack
            .iter()
            .chain(&self.nodes)
            .find_map(|(node_id, node)| (*node_id == id).then_some(node))
    }

    pub(crate) fn parent_id(&self, id: NodeId) -> Option<NodeId> {
        self.parents.get(&id).copied()
    }

    #[cfg(test)]
    pub(crate) fn node_is_focused(&self, id: NodeId) -> bool {
        self.focus == Some(id)
    }

    pub(crate) fn focus_is_ancestor_of_current(&self) -> bool {
        self.focus.is_some_and(|focus| {
            self.stack
                .iter()
                .take(self.stack.len().saturating_sub(1))
                .any(|(id, _)| *id == focus)
        })
    }

    pub(crate) fn set_focus(&mut self, id: NodeId) {
        if self.focus.is_some_and(|existing| existing != id) {
            log::warn!("Multiple elements claimed accessibility focus; using {id:?}");
        }
        self.focus = Some(id);
    }

    pub(crate) fn set_active_descendant(&mut self, id: NodeId) {
        if self
            .active_descendant
            .is_some_and(|existing| existing != id)
        {
            log::warn!(
                "Multiple elements claimed the accessibility active descendant; using {id:?}"
            );
        }
        self.active_descendant = Some(id);
    }

    pub(crate) fn finalize(&mut self) -> TreeUpdate {
        if self.stack.is_empty() && self.nodes.is_empty() {
            self.begin_frame(None);
        }
        if self.stack.len() > 1 {
            log::error!(
                "Accessibility traversal left {} unclosed elements",
                self.stack.len() - 1
            );
        }
        while let Some(node) = self.stack.pop() {
            self.nodes.push(node);
        }

        // Custom elements can refine child references directly. Keep their
        // references rooted, unique and acyclic before AccessKit consumes them.
        let indices: HashMap<_, _> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(index, (id, _))| (*id, index))
            .collect();
        let mut reachable = HashSet::from([ROOT_NODE_ID]);
        let mut pending = vec![ROOT_NODE_ID];
        let mut parents = HashMap::new();
        while let Some(id) = pending.pop() {
            let Some(index) = indices.get(&id).copied() else {
                continue;
            };
            let mut children = Vec::new();
            for child in self.nodes[index].1.children().iter().copied() {
                if indices.contains_key(&child) && reachable.insert(child) {
                    children.push(child);
                    parents.insert(child, id);
                    pending.push(child);
                } else {
                    log::warn!(
                        "Invalid or repeated accessibility child {child:?} on {id:?} was rejected"
                    );
                }
            }
            self.nodes[index].1.set_children(children);
        }
        self.nodes.retain(|(id, _)| reachable.contains(id));
        self.seen_ids = reachable;
        self.parents = parents;
        if let Some(id) = self.focus.filter(|id| !self.has_node(*id)) {
            log::warn!("Accessibility focus {id:?} is absent from the completed tree");
        }
        if let Some(id) = self.active_descendant.filter(|id| !self.has_node(*id)) {
            log::warn!("Accessibility active descendant {id:?} is absent from the completed tree");
        }
        let focus = self
            .active_descendant
            .filter(|id| self.has_node(*id))
            .or(self.focus.filter(|id| self.has_node(*id)))
            .unwrap_or(ROOT_NODE_ID);
        TreeUpdate {
            nodes: self.nodes.clone(),
            tree: Some(TreeInfo::new(ROOT_NODE_ID)),
            tree_id: TreeId::ROOT,
            focus,
        }
    }
}

/// A listener for an accessibility action dispatched to an element or its child.
pub type A11yActionListener =
    Box<dyn FnMut(Option<&accesskit::ActionData>, &mut crate::Window, &mut crate::App)>;

#[cfg(test)]
mod tests {
    use super::{
        A11yNodeBuilder, A11ySubtreeBuilder, ROOT_NODE_ID, SYNTHETIC_NAMESPACE,
        node_id_for_element, node_id_for_ordinal,
    };
    use crate::{ElementId, GlobalElementId, SharedString};
    use accesskit::{Action, Node, NodeId, Role, TreeId};
    use accesskit_consumer::{NodeRef, Tree, TreeChangeHandler, TreeState};

    fn builder() -> A11yNodeBuilder {
        let mut builder = A11yNodeBuilder::new();
        builder.begin_frame(Some(&SharedString::new_static("Test window")));
        builder
    }

    fn node(state: &TreeState, id: NodeId) -> NodeRef<'_> {
        state
            .node_by_tree_local_id(id, TreeId::ROOT)
            .expect("node in AccessKit tree")
    }

    #[derive(Default)]
    struct Changes {
        removed: usize,
        focus_moves: usize,
    }

    impl TreeChangeHandler for Changes {
        fn node_added(&mut self, _: &NodeRef) {}
        fn node_updated(&mut self, _: &NodeRef, _: &NodeRef) {}
        fn focus_moved(&mut self, _: Option<&NodeRef>, _: Option<&NodeRef>) {
            self.focus_moves += 1;
        }
        fn node_removed(&mut self, _: &NodeRef) {
            self.removed += 1;
        }
    }

    #[test]
    fn stable_ids_separate_parents_and_synthetic_nodes() {
        let element = GlobalElementId([ElementId::from("editor")].into());
        let parent = node_id_for_element(&element);
        assert_ne!(parent, ROOT_NODE_ID);
        assert_eq!(parent.0 & SYNTHETIC_NAMESPACE, 0);
        assert_eq!(node_id_for_element(&element), parent);
        let owned_element = GlobalElementId([ElementId::from(String::from("editor"))].into());
        assert_eq!(node_id_for_element(&owned_element), parent);
        assert_ne!(node_id_for_ordinal(0), parent);
        let mut nodes = builder();
        assert!(nodes.push(parent, Node::new(Role::TextInput)));
        let first = {
            let subtree = A11ySubtreeBuilder::new(parent, &mut nodes).expect("current element");
            let id = subtree.synthetic_node_id(("line", 0));
            assert_eq!(subtree.synthetic_node_id(("line", 0)), id);
            assert_ne!(subtree.synthetic_node_id(("line", 1)), id);
            assert_ne!(id.0 & SYNTHETIC_NAMESPACE, 0);
            id
        };
        nodes.pop();
        let other_parent = node_id_for_ordinal(1);
        assert!(nodes.push(other_parent, Node::new(Role::TextInput)));
        let subtree =
            A11ySubtreeBuilder::new(other_parent, &mut nodes).expect("second current element");
        assert_ne!(subtree.synthetic_node_id(("line", 0)), first);
    }

    #[test]
    fn synthetic_nodes_and_parent_metadata_reach_accesskit_consumer() {
        let mut nodes = builder();
        let parent = node_id_for_ordinal(1);
        assert!(nodes.push(parent, Node::new(Role::TextInput)));
        nodes.set_focus(parent);
        let child = {
            let mut subtree = A11ySubtreeBuilder::new(parent, &mut nodes).expect("current element");
            subtree.parent_node().set_label("Message");
            let child = subtree.synthetic_node_id("first-line");
            let mut child_node = Node::new(Role::Label);
            child_node.set_value("Hello 世界");
            child_node.add_action(Action::SetTextSelection);
            assert!(subtree.push_child(child, child_node));
            assert!(!subtree.push_child(child, Node::new(Role::Button)));
            child
        };
        nodes.pop();
        let tree = Tree::new(nodes.finalize(), true);
        let state = tree.state();
        let parent_node = node(state, parent);
        assert_eq!(parent_node.label().as_deref(), Some("Message"));
        let child_node = node(state, child);
        assert_eq!(child_node.parent_id(), Some(parent_node.id()));
        assert_eq!(child_node.value().as_deref(), Some("Hello 世界"));
        assert!(child_node.data().supports_action(Action::SetTextSelection));
        assert_eq!(nodes.parent_id(child), Some(parent));
        assert_eq!(state.focus_id(), Some(parent_node.id()));
    }

    #[test]
    fn active_descendant_follows_a_focused_ancestor_across_frames() {
        let mut nodes = builder();
        let container = node_id_for_ordinal(1);
        let child = node_id_for_ordinal(2);
        assert!(nodes.push(container, Node::new(Role::ListBox)));
        nodes.set_focus(container);
        assert!(!nodes.focus_is_ancestor_of_current());
        assert!(nodes.push(child, Node::new(Role::ListBoxOption)));
        assert!(nodes.focus_is_ancestor_of_current());
        nodes.set_active_descendant(child);
        nodes.pop();
        nodes.pop();
        let mut tree = Tree::new(nodes.finalize(), true);
        assert_eq!(
            tree.state().focus_id(),
            Some(node(tree.state(), child).id())
        );
        nodes.begin_frame(None);
        assert!(!nodes.has_node(child));
        assert_eq!(nodes.parent_id(child), None);
        assert!(nodes.push_leaf(container, Node::new(Role::Button)));
        nodes.set_focus(container);
        let mut changes = Changes::default();
        tree.update_and_process_changes(nodes.finalize(), &mut changes);
        assert!(
            tree.state()
                .node_by_tree_local_id(child, TreeId::ROOT)
                .is_none()
        );
        assert_eq!(
            tree.state().focus_id(),
            Some(node(tree.state(), container).id())
        );
        assert_eq!(changes.removed, 1);
        assert_eq!(changes.focus_moves, 1);
        assert!(nodes.node_is_focused(container));
    }

    #[test]
    fn duplicate_roots_and_unbalanced_or_invalid_children_stay_valid() {
        let mut nodes = builder();
        assert!(!nodes.push_leaf(ROOT_NODE_ID, Node::new(Role::Button)));
        let child = node_id_for_ordinal(1);
        assert!(nodes.push(child, Node::new(Role::Group)));
        assert!(A11ySubtreeBuilder::new(NodeId(999), &mut nodes).is_none());
        nodes
            .current_node_mut()
            .expect("child")
            .set_children([ROOT_NODE_ID, NodeId(999), child]);
        nodes.set_focus(NodeId(999));
        let mut tree = Tree::new(nodes.finalize(), true);
        assert!(node(tree.state(), child).children().next().is_none());
        assert_eq!(tree.state().focus_id(), Some(tree.state().root_id()));
        tree.update_and_process_changes(nodes.finalize(), &mut Changes::default());
        assert!(
            tree.state()
                .node_by_tree_local_id(child, TreeId::ROOT)
                .is_some()
        );
    }
}
