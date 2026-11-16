//! Positive regressions for ordinary uncached accessibility action-route restoration.
use super::*;
use crate::{InteractiveElement, IntoElement, Styled, TestAppContext, div};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RouteEvent {
    Created { token: usize, revision: usize },
    Invoked { token: usize, revision: usize },
    Dropped { token: usize, revision: usize },
    Ordinal { token: usize, ordinal: usize },
}

type RouteLedger = Rc<RefCell<Vec<RouteEvent>>>;

struct RouteCapture {
    token: usize,
    revision: usize,
    ledger: RouteLedger,
}

impl RouteCapture {
    fn record_invocation(&self) {
        self.ledger.borrow_mut().push(RouteEvent::Invoked {
            token: self.token,
            revision: self.revision,
        });
    }
}

impl Drop for RouteCapture {
    fn drop(&mut self) {
        self.ledger.borrow_mut().push(RouteEvent::Dropped {
            token: self.token,
            revision: self.revision,
        });
    }
}

struct UncachedRedrawProbe {
    revision: usize,
    redraw_revision: usize,
    next_token: Rc<Cell<usize>>,
    fired: Rc<Cell<bool>>,
    remove_on_first: bool,
    removed: bool,
    ledger: RouteLedger,
}

impl Render for UncachedRedrawProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.removed {
            return div().id("uncached-action-redraw-root");
        }
        let token = self.next_token.get() + 1;
        self.next_token.set(token);
        let revision = self.revision;
        self.ledger
            .borrow_mut()
            .push(RouteEvent::Created { token, revision });
        let capture = RouteCapture {
            token,
            revision,
            ledger: self.ledger.clone(),
        };
        let owner = cx.entity().downgrade();
        let fired = self.fired.clone();
        let redraw_revision = self.redraw_revision;
        let remove_on_first = self.remove_on_first;
        div()
            .id("uncached-action-redraw-probe")
            .w(px(100.))
            .h(px(40.))
            .role(accesskit::Role::Button)
            .aria_label("uncached-action-redraw-probe")
            .aria_value(revision.to_string())
            .on_a11y_action(accesskit::Action::Increment, {
                let mut ordinal = 0usize;
                move |_, window, cx| {
                    ordinal += 1;
                    capture.record_invocation();
                    capture.ledger.borrow_mut().push(RouteEvent::Ordinal {
                        token: capture.token,
                        ordinal,
                    });
                    if !fired.replace(true) {
                        let owner = owner.upgrade().expect("owner retained by test");
                        owner.update(cx, |owner, cx| {
                            if remove_on_first {
                                owner.removed = true;
                            } else {
                                owner.revision = redraw_revision;
                            }
                            cx.notify();
                        });
                        window.draw(cx).clear();
                    }
                }
            })
    }
}

fn action_request(target_node: accesskit::NodeId) -> accesskit::ActionRequest {
    accesskit::ActionRequest {
        action: accesskit::Action::Increment,
        target_tree: accesskit::TreeId::ROOT,
        target_node,
        data: None,
    }
}

fn probe_node(update: &accesskit::TreeUpdate) -> (accesskit::NodeId, String) {
    let mut matches = update
        .nodes
        .iter()
        .filter(|(_, node)| node.label() == Some("uncached-action-redraw-probe"));
    let (id, node) = matches.next().expect("published probe node");
    assert!(matches.next().is_none(), "one published probe node");
    assert_eq!(node.role(), accesskit::Role::Button);
    assert!(node.supports_action(accesskit::Action::Increment));
    (*id, node.value().expect("published revision").to_owned())
}

fn completed_callback_draw_preserves_published_route(
    cx: &mut TestAppContext,
    redraw_revision: usize,
) {
    let ledger: RouteLedger = Rc::default();
    let fired = Rc::new(Cell::new(false));
    let next_token = Rc::new(Cell::new(0));
    let (_owner, cx) = cx.add_window_view({
        let ledger = ledger.clone();
        let fired = fired.clone();
        let next_token = next_token.clone();
        move |_, _| UncachedRedrawProbe {
            revision: 1,
            redraw_revision,
            next_token,
            fired,
            remove_on_first: false,
            removed: false,
            ledger,
        }
    });
    cx.simulate_resize(size(px(100.), px(40.)));
    cx.run_until_parked();
    cx.activate_accessibility();
    cx.run_until_parked();
    let (target, value) = probe_node(&cx.accessibility_tree().expect("initial tree"));
    assert_eq!(value, "1");
    ledger.borrow_mut().clear();

    // Both direct requests run in one update, before any later effect-flush draw.
    let (published, events) = cx.update(|window, cx| {
        window.dispatch_a11y_action(action_request(target), cx);
        let published = window
            .platform_window
            .as_test()
            .expect("test platform")
            .0
            .lock()
            .accessibility_tree
            .clone()
            .expect("callback draw published a tree");
        window.dispatch_a11y_action(action_request(target), cx);
        (published, ledger.borrow().clone())
    });

    let (after_target, after_value) = probe_node(&published);
    assert_eq!(
        after_target, target,
        "same logical owner retains its node ID"
    );
    assert_eq!(after_value, redraw_revision.to_string());
    let calls: Vec<_> = events
        .iter()
        .filter_map(|event| match *event {
            RouteEvent::Invoked { token, revision } => Some((token, revision)),
            _ => None,
        })
        .collect();
    let new_routes: Vec<_> = events
        .iter()
        .filter_map(|event| match *event {
            RouteEvent::Created { token, revision } => Some((token, revision)),
            _ => None,
        })
        .collect();
    assert_eq!(
        calls.len(),
        2,
        "two direct requests invoked exactly two callbacks"
    );
    assert_eq!(
        new_routes.len(),
        1,
        "callback draw created one replacement route"
    );
    let old = calls[0];
    let new = new_routes[0];
    assert_eq!(old.1, 1);
    assert_eq!(new.1, redraw_revision);
    assert_ne!(
        old.0, new.0,
        "uncached redraw created a distinct callback body"
    );
    assert!(
        !events.contains(&RouteEvent::Dropped {
            token: new.0,
            revision: new.1,
        }),
        "replacement route survives through the second request observation"
    );
    assert!(
        events.contains(&RouteEvent::Dropped {
            token: old.0,
            revision: old.1
        }),
        "leased old listener vector is discarded after completed draw"
    );
    assert_eq!(
        calls[1], new,
        "second request reaches the callback published by completed draw"
    );
    assert_ne!(
        calls[1], old,
        "old callback vector was not restored"
    );
    let first_call = events
        .iter()
        .position(|event| {
            *event
                == RouteEvent::Invoked {
                    token: old.0,
                    revision: old.1,
                }
        })
        .expect("first invocation");
    let created = events
        .iter()
        .position(|event| {
            *event
                == RouteEvent::Created {
                    token: new.0,
                    revision: new.1,
                }
        })
        .expect("new route creation");
    let old_drop = events
        .iter()
        .position(|event| {
            *event
                == RouteEvent::Dropped {
                    token: old.0,
                    revision: old.1,
                }
        })
        .expect("old leased route drop");
    let second_call = events
        .iter()
        .position(|event| {
            *event
                == RouteEvent::Invoked {
                    token: new.0,
                    revision: new.1,
                }
        })
        .expect("replacement route invocation");
    assert!(
        first_call < created && created < old_drop && old_drop < second_call,
        "first invocation, replacement creation, old drop, then replacement invocation"
    );
    // The replacement remains installed through this observation window; its later
    // teardown drop is outside the bounded ledger assertion.
}

#[wgpui::test]
fn uncached_callback_draw_preserves_new_route_for_same_revision(cx: &mut TestAppContext) {
    completed_callback_draw_preserves_published_route(cx, 1);
}

#[wgpui::test]
fn uncached_callback_draw_preserves_new_route_for_new_revision(cx: &mut TestAppContext) {
    completed_callback_draw_preserves_published_route(cx, 2);
}

#[wgpui::test]
fn uncached_no_draw_retains_fnmut_state(cx: &mut TestAppContext) {
    let ledger: RouteLedger = Rc::default();
    let fired = Rc::new(Cell::new(true));
    let next_token = Rc::new(Cell::new(0));
    let (_owner, cx) = cx.add_window_view({
        let ledger = ledger.clone();
        let fired = fired.clone();
        let next_token = next_token.clone();
        move |_, _| UncachedRedrawProbe {
            revision: 1,
            redraw_revision: 1,
            next_token,
            fired,
            remove_on_first: false,
            removed: false,
            ledger,
        }
    });
    cx.simulate_resize(size(px(100.), px(40.)));
    cx.run_until_parked();
    cx.activate_accessibility();
    cx.run_until_parked();
    let (target, _) = probe_node(&cx.accessibility_tree().expect("initial tree"));
    ledger.borrow_mut().clear();
    let events = cx.update(|window, cx| {
        window.dispatch_a11y_action(action_request(target), cx);
        window.dispatch_a11y_action(action_request(target), cx);
        ledger.borrow().clone()
    });
    let ordinals: Vec<_> = events
        .iter()
        .filter_map(|event| match *event {
            RouteEvent::Ordinal { token, ordinal } => Some((token, ordinal)),
            _ => None,
        })
        .collect();
    assert_eq!(ordinals.len(), 2);
    assert_eq!(ordinals[0].0, ordinals[1].0, "same retained FnMut callback");
    assert_eq!([ordinals[0].1, ordinals[1].1], [1, 2]);
    assert!(
        !events.iter().any(|event| {
            matches!(event, RouteEvent::Created { .. } | RouteEvent::Dropped { .. })
        }),
        "no draw creates or drops a route in the bounded observation"
    );
}

#[wgpui::test]
fn completed_callback_draw_removal_does_not_restore_stale_route(cx: &mut TestAppContext) {
    let ledger: RouteLedger = Rc::default();
    let fired = Rc::new(Cell::new(false));
    let next_token = Rc::new(Cell::new(0));
    let (_owner, cx) = cx.add_window_view({
        let ledger = ledger.clone();
        let fired = fired.clone();
        let next_token = next_token.clone();
        move |_, _| UncachedRedrawProbe {
            revision: 1,
            redraw_revision: 1,
            next_token,
            fired,
            remove_on_first: true,
            removed: false,
            ledger,
        }
    });
    cx.simulate_resize(size(px(100.), px(40.)));
    cx.run_until_parked();
    cx.activate_accessibility();
    cx.run_until_parked();
    let (target, _) = probe_node(&cx.accessibility_tree().expect("initial tree"));
    ledger.borrow_mut().clear();

    let (published, events) = cx.update(|window, cx| {
        window.dispatch_a11y_action(action_request(target), cx);
        let published = window
            .platform_window
            .as_test()
            .expect("test platform")
            .0
            .lock()
            .accessibility_tree
            .clone()
            .expect("removal draw published tree");
        window.dispatch_a11y_action(action_request(target), cx);
        (published, ledger.borrow().clone())
    });

    let nodes: HashMap<_, _> = published
        .nodes
        .iter()
        .map(|(id, node)| (*id, node))
        .collect();
    let root = published
        .tree
        .as_ref()
        .expect("published tree metadata")
        .root;
    let mut reachable = HashSet::new();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        assert!(reachable.insert(id), "tree contains no repeated reachable node");
        let node = nodes
            .get(&id)
            .expect("reachable child exists in published nodes");
        pending.extend(node.children().iter().copied());
    }
    assert!(
        !reachable.contains(&target),
        "removed target is absent from rooted published tree"
    );
    let calls: Vec<_> = events
        .iter()
        .filter_map(|event| match *event {
            RouteEvent::Invoked { token, revision } => Some((token, revision)),
            _ => None,
        })
        .collect();
    assert_eq!(calls.len(), 1, "second request does not invoke stale callback");
    let old = calls[0];
    assert!(
        events.contains(&RouteEvent::Dropped {
            token: old.0,
            revision: old.1,
        }),
        "leased callback capture is dropped after completed removal draw"
    );
}
