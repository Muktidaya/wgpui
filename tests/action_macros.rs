use schemars::JsonSchema;
use serde::Deserialize;
use wgpui::{Action, actions, register_action};

actions!(test_only, [SomeAction, SomeActionWithDocs]);

#[derive(PartialEq, Clone, Deserialize, JsonSchema, Action)]
#[action(namespace = test_only)]
#[serde(deny_unknown_fields)]
struct AnotherAction {
    count: usize,
}

#[derive(PartialEq, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterableAction {}

register_action!(RegisterableAction);

impl Action for RegisterableAction {
    fn boxed_clone(&self) -> Box<dyn Action> {
        Box::new(self.clone())
    }
    fn partial_eq(&self, other: &dyn Action) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
    fn name(&self) -> &'static str {
        Self::name_for_type()
    }
    fn name_for_type() -> &'static str {
        "test_only::RegisterableAction"
    }
    fn build(value: serde_json::Value) -> anyhow::Result<Box<dyn Action>> {
        Ok(Box::new(serde_json::from_value::<Self>(value)?))
    }
}

#[test]
fn action_macros_build_and_register_typed_actions() -> anyhow::Result<()> {
    assert_eq!(SomeAction.name(), "test_only::SomeAction");
    let action = AnotherAction::build(serde_json::json!({"count": 3}))?;
    assert!(action.partial_eq(&AnotherAction { count: 3 }));
    assert!(!action.partial_eq(&AnotherAction { count: 4 }));
    assert!(action.partial_eq(action.boxed_clone().as_ref()));
    assert!(AnotherAction::build(serde_json::json!({"unexpected": true})).is_err());
    let registration = wgpui::private::inventory::iter::<wgpui::MacroActionBuilder>
        .into_iter()
        .map(|builder| builder.0())
        .find(|data| data.name == RegisterableAction::name_for_type())
        .expect("register_action must submit its builder");
    assert!((registration.build)(serde_json::json!({}))?.partial_eq(&RegisterableAction {}));
    Ok(())
}
