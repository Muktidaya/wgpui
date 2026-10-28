//! Disabled semantics on a control's accessibility node.
//!
//! GPUI has no `aria_disabled` setter, so a disabled control would reach
//! assistive technology as one that accepts input. [`A11yDisabledExt`] wraps the
//! control's root element without adding a layout node, a focus stop or an
//! accessibility node, and only marks the node it writes disabled.
use wgpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, InteractiveElement,
    Interactivity, IntoElement, LayoutId, Pixels, Window, accesskit,
};

/// A control's root element whose accessibility node may be marked disabled.
pub(crate) struct A11yDisabled<E> {
    inner: E,
    disabled: bool,
}

impl<E> A11yDisabled<E> {
    pub(crate) fn new(inner: E, disabled: bool) -> Self {
        Self { inner, disabled }
    }
}

/// Marks a control's accessibility node disabled while `disabled` is true.
///
/// Call it at the end of the control's builder chain. With `test-support`, the
/// wrapper goes inside the observation, so its snapshot reports the flag too.
pub(crate) trait A11yDisabledExt: Sized {
    type Output: IntoElement;

    fn a11y_disabled(self, disabled: bool) -> Self::Output;
}

#[cfg(not(feature = "test-support"))]
impl<E: Element + InteractiveElement> A11yDisabledExt for E {
    type Output = A11yDisabled<E>;

    fn a11y_disabled(self, disabled: bool) -> Self::Output {
        A11yDisabled::new(self, disabled)
    }
}

#[cfg(feature = "test-support")]
impl<E: Element + InteractiveElement> A11yDisabledExt for wgpui::Stateful<E> {
    type Output = A11yDisabled<Self>;

    fn a11y_disabled(self, disabled: bool) -> Self::Output {
        A11yDisabled::new(self, disabled)
    }
}

#[cfg(feature = "test-support")]
impl<E: Element<PrepaintState = Option<wgpui::Hitbox>> + InteractiveElement> A11yDisabledExt
    for crate::test_support::Observed<E>
{
    type Output = crate::test_support::Observed<A11yDisabled<E>>;

    fn a11y_disabled(self, disabled: bool) -> Self::Output {
        self.map_inner(|inner| A11yDisabled::new(inner, disabled))
    }
}

impl<E: Element> IntoElement for A11yDisabled<E> {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<E: InteractiveElement> InteractiveElement for A11yDisabled<E> {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.inner.interactivity()
    }
}

impl<E: Element> Element for A11yDisabled<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.inner.id()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.inner.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.inner
            .prepaint(id, inspector, bounds, layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner
            .paint(id, inspector, bounds, layout, prepaint, window, cx)
    }

    fn a11y_role(&self) -> Option<wgpui::Role> {
        self.inner.a11y_role()
    }

    fn take_a11y_actions(&mut self) -> Vec<(accesskit::Action, wgpui::A11yActionListener)> {
        self.inner.take_a11y_actions()
    }

    fn a11y_focus_handle(&self) -> Option<wgpui::FocusHandle> {
        self.inner.a11y_focus_handle()
    }

    fn a11y_reports_active_descendant(&self) -> bool {
        self.inner.a11y_reports_active_descendant()
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        self.inner.write_a11y_info(node);
        if self.disabled {
            node.set_disabled();
        }
    }

    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Self::PrepaintState,
        builder: &mut wgpui::A11ySubtreeBuilder,
    ) {
        self.inner.a11y_synthetic_children(prepaint, builder);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::{cell::RefCell, rc::Rc};

    use wgpui::{
        AppContext as _, Context, IntoElement, Render, RenderOnce, TestAppContext, canvas,
    };

    use super::*;
    use crate::{
        Button, Checkbox, DatePicker, Link, Radio, Select, Slider, Switch, Tab, TimeField,
        TimeFieldState, Toggle, slider::SliderState,
    };

    struct SemanticProbe {
        focus: wgpui::FocusHandle,
        values: Rc<RefCell<Vec<String>>>,
        disabled: bool,
    }

    impl Render for SemanticProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            use crate::{FocusTrapElement as _, TestSupportExt as _};
            use wgpui::{StatefulInteractiveElement as _, Styled as _, div};

            let values = self.values.clone();
            let field = div()
                .id("wrapped-field")
                .size_full()
                .role(accesskit::Role::TextInput)
                .aria_label("Semantic wrapped field")
                .track_focus(&self.focus)
                .on_a11y_action(accesskit::Action::SetValue, move |data, _, _| {
                    if let Some(accesskit::ActionData::Value(value)) = data {
                        values.borrow_mut().push(value.to_string());
                    }
                })
                .a11y_synthetic_children(|builder| {
                    let id = builder.synthetic_node_id("line");
                    let mut node = accesskit::Node::new(accesskit::Role::Label);
                    node.set_value("Wrapped text");
                    assert!(builder.push_child(id, node));
                })
                .focus_trap("semantic-trap", &self.focus);
            A11yDisabled::new(field, self.disabled).test_support()
        }
    }

    #[wgpui::test]
    fn wrapped_semantics_deliver_native_actions_and_respect_disabled_ancestors(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::init);
        for disabled in [false, true] {
            let values = Rc::new(RefCell::new(Vec::new()));
            let (root, cx) = cx.add_window_view({
                let values = values.clone();
                move |_, cx| SemanticProbe {
                    focus: cx.focus_handle(),
                    values,
                    disabled,
                }
            });
            cx.activate_accessibility();
            let tree = cx
                .accessibility_tree()
                .expect("rendered native semantic tree");
            let (_, field) = tree
                .nodes
                .iter()
                .find(|(_, node)| node.label() == Some("Semantic wrapped field"))
                .expect("wrapped field is exposed");
            assert_eq!(field.is_disabled(), disabled);
            assert!(field.supports_action(accesskit::Action::SetValue));
            let child = *field
                .children()
                .first()
                .expect("synthetic child survives every wrapper");
            assert!(
                tree.nodes
                    .iter()
                    .any(|(id, node)| *id == child && node.value() == Some("Wrapped text"))
            );

            cx.simulate_a11y_action(accesskit::ActionRequest {
                action: accesskit::Action::SetValue,
                target_tree: accesskit::TreeId::ROOT,
                target_node: child,
                data: Some(accesskit::ActionData::Value("changed 世界".into())),
            });
            cx.simulate_a11y_action(accesskit::ActionRequest {
                action: accesskit::Action::Focus,
                target_tree: accesskit::TreeId::ROOT,
                target_node: child,
                data: None,
            });
            let expected_values = if disabled {
                Vec::new()
            } else {
                vec![String::from("changed 世界")]
            };
            assert_eq!(*values.borrow(), expected_values);
            assert_eq!(
                cx.update(|window, cx| root.read(cx).focus.is_focused(window)),
                !disabled
            );
        }
    }

    /// The accessibility node a rendered control writes.
    fn node_of(element: impl IntoElement) -> accesskit::Node {
        let element = element.into_element();
        let mut node = accesskit::Node::new(element.a11y_role().unwrap_or(wgpui::Role::Unknown));
        element.write_a11y_info(&mut node);
        node
    }

    type Captured = Arc<Mutex<Vec<(&'static str, bool, accesskit::Node)>>>;

    struct Probe {
        captured: Captured,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let captured = self.captured.clone();
            canvas(
                move |_, window, cx| {
                    let slider = cx.new(|_| SliderState::new());
                    let time = cx.new(|cx| TimeFieldState::new(window, cx));
                    let focus = cx.focus_handle();
                    let mut nodes = captured.lock().unwrap();
                    nodes.clear();
                    for disabled in [false, true] {
                        nodes.push((
                            "button",
                            disabled,
                            node_of(Button::new("button").disabled(disabled).render(window, cx)),
                        ));
                        nodes.push((
                            "link",
                            disabled,
                            node_of(Link::new("link").disabled(disabled).render(window, cx)),
                        ));
                        nodes.push((
                            "switch",
                            disabled,
                            node_of(Switch::new("switch").disabled(disabled).render(window, cx)),
                        ));
                        nodes.push((
                            "radio",
                            disabled,
                            node_of(Radio::new("radio").disabled(disabled).render(window, cx)),
                        ));
                        nodes.push((
                            "toggle",
                            disabled,
                            node_of(Toggle::new("toggle").disabled(disabled).render(window, cx)),
                        ));
                        nodes.push((
                            "checkbox",
                            disabled,
                            node_of(
                                Checkbox::new("checkbox")
                                    .disabled(disabled)
                                    .render(window, cx),
                            ),
                        ));
                        nodes.push((
                            "select",
                            disabled,
                            node_of(Select::new("select").disabled(disabled).render(window, cx)),
                        ));
                        nodes.push((
                            "tab",
                            disabled,
                            node_of(Tab::new("tab").disabled(disabled).render(window, cx)),
                        ));
                        nodes.push((
                            "date_picker",
                            disabled,
                            node_of(
                                DatePicker::new("date-picker", &focus)
                                    .disabled(disabled)
                                    .render(window, cx),
                            ),
                        ));
                        nodes.push((
                            "time_field",
                            disabled,
                            node_of(
                                TimeField::new("time-field", &time)
                                    .disabled(disabled)
                                    .render(window, cx),
                            ),
                        ));
                        nodes.push((
                            "slider",
                            disabled,
                            node_of(Slider::new(&slider).disabled(disabled).render(window, cx)),
                        ));
                    }
                },
                |_, _, _, _| {},
            )
        }
    }

    #[wgpui::test]
    fn a_disabled_control_marks_its_node_disabled(cx: &mut TestAppContext) {
        let captured: Captured = Arc::default();
        let (_, cx) = cx.add_window_view({
            let captured = captured.clone();
            move |_, _| Probe { captured }
        });
        cx.update(|window, cx| window.draw(cx).clear());

        let nodes = captured.lock().unwrap();
        assert_eq!(nodes.len(), 22);
        for (name, disabled, node) in nodes.iter() {
            assert_ne!(node.role(), wgpui::Role::Unknown, "{name} wrote no node");
            assert_eq!(node.is_disabled(), *disabled, "{name}, disabled {disabled}");
            if *name == "slider" {
                // Nor does assistive technology move a disabled slider.
                assert_eq!(
                    node.supports_action(accesskit::Action::Increment),
                    !disabled,
                    "slider, disabled {disabled}"
                );
            }
        }
    }

    struct Observed {
        disabled: bool,
    }

    impl Render for Observed {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Switch::new("switch").disabled(self.disabled)
        }
    }

    #[wgpui::test]
    fn the_test_support_snapshot_sees_the_flag(cx: &mut TestAppContext) {
        for disabled in [false, true] {
            let (_, cx) = cx.add_window_view(move |_, _| Observed { disabled });
            cx.update(|window, cx| {
                window.draw(cx).clear();
                let snapshot = crate::test_support::find(window, &[], &"switch".into())
                    .expect("switch is observed");
                assert_eq!(snapshot.disabled(), disabled.then_some(true));
            });
        }
    }
}
