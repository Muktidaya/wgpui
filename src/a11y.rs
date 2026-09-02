#![allow(missing_docs)]

//! Accessibility property stubs for gpui-component compatibility.

use crate::SharedString;

/// Author-facing ARIA-like properties stored on interactive elements.
#[derive(Clone, Debug, Default)]
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

/// Builder for synthetic accessibility children (stub).
#[derive(Default)]
pub struct A11ySubtreeBuilder;

impl A11ySubtreeBuilder {
    pub fn synthetic_node_id(&mut self, _index: usize) -> accesskit::NodeId {
        accesskit::NodeId(0)
    }

    pub fn push_child(&mut self, _id: accesskit::NodeId, _node: accesskit::Node) {}

    pub fn parent_node(&mut self) -> &mut accesskit::Node {
        panic!("A11ySubtreeBuilder::parent_node is not implemented in wgpui yet")
    }
}

pub type A11yActionListener = Box<
    dyn FnMut(Option<&accesskit::ActionData>, &mut crate::Window, &mut crate::App),
>;
