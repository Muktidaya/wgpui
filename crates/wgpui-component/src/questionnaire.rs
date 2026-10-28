//! Composable questionnaire controls.
//!
//! The behavior — answers, validation, navigation, focus and shortcuts — lives
//! in [`wgpui_base::questionnaire`]; this module is its skin. The public path
//! stays `wgpui_component::questionnaire::*` for both halves.

mod components;

pub use components::*;
pub use wgpui_base::questionnaire::{
    QuestionnaireAnswer, QuestionnaireAnswerChange, QuestionnaireAnswers,
    QuestionnaireChoiceDefinition, QuestionnaireChoiceState, QuestionnaireEvent,
    QuestionnaireInputDefinition, QuestionnaireItemDefinition, QuestionnaireItemState,
    QuestionnaireItemStatus, QuestionnaireNavigationState, QuestionnaireProgressState,
    QuestionnaireSchemaError, QuestionnaireShortcutMode, QuestionnaireState,
    QuestionnaireSubmission, QuestionnaireSubmissionItem, QuestionnaireValidationContext,
    QuestionnaireValidationError, QuestionnaireValidator,
};

pub(crate) fn init(_: &mut wgpui::App) {}
