//! The Windows recognizer: continuous dictation with
//! `Windows.Media.SpeechRecognition`.
//!
//! The WinRT recognizer captures from the default microphone itself and has no
//! way to consume audio from elsewhere, so the pushed PCM is ignored; the
//! [`Microphone`](crate::speech::Microphone) keeps capturing alongside it only
//! to drive the waveform, which WASAPI's shared mode allows.
//!
//! The speech objects are agile and are called from the foreground thread.
//! Their completions and events arrive on WinRT threads, which only
//! forward them over a channel to a foreground task that reports to the sink.

use anyhow::anyhow;
use smol::channel::{Receiver, Sender, unbounded};
use wgpui::{App, AsyncApp, SharedString, Task};
use windows::{
    Foundation::TypedEventHandler,
    Globalization::Language,
    Media::SpeechRecognition::{
        SpeechContinuousRecognitionCompletedEventArgs,
        SpeechContinuousRecognitionResultGeneratedEventArgs, SpeechContinuousRecognitionSession,
        SpeechRecognitionConfidence, SpeechRecognitionHypothesisGeneratedEventArgs,
        SpeechRecognitionResult, SpeechRecognitionResultStatus, SpeechRecognitionScenario,
        SpeechRecognitionTopicConstraint, SpeechRecognizer as WinSpeechRecognizer,
    },
    Win32::Foundation::E_ACCESSDENIED,
    core::{HRESULT, HSTRING, Ref},
};

use crate::speech::{AudioFormat, RecognitionSession, SpeechError, SpeechRecognizer, SpeechSink};

/// `SPERR_SPEECH_PRIVACY_POLICY_NOT_ACCEPTED`: "Online speech recognition" is
/// turned off in the privacy settings, which dictation requires.
const PRIVACY_POLICY_NOT_ACCEPTED: HRESULT = HRESULT(0x80045509_u32 as i32);
/// `MF_E_NO_CAPTURE_DEVICES_AVAILABLE`.
const NO_CAPTURE_DEVICES: HRESULT = HRESULT(0xC00DABE0_u32 as i32);

pub(super) struct PlatformRecognizer {
    /// The dictation language, or `None` when the requested one is malformed or
    /// has no speech pack installed.
    language: Option<Language>,
    separator: &'static str,
}

impl PlatformRecognizer {
    pub(super) fn new(locale: Option<SharedString>) -> Self {
        let language = supported_language(locale.as_deref())
            .inspect_err(|error| log::warn!("speech: no dictation language: {error:#}"))
            .ok()
            .flatten();
        let separator = language
            .as_ref()
            .and_then(|language| match language.LanguageTag() {
                Ok(tag) => Some(tag),
                Err(error) => {
                    tracing::warn!("Cannot read the speech recognizer language tag: {error}");
                    None
                }
            })
            .map_or(" ", |tag| phrase_separator(&tag.to_string_lossy()));
        Self {
            language,
            separator,
        }
    }
}

impl SpeechRecognizer for PlatformRecognizer {
    fn audio_format(&self) -> AudioFormat {
        AudioFormat::default()
    }

    fn is_available(&self, _: &App) -> bool {
        self.language.is_some()
    }

    fn start(
        &self,
        sink: SpeechSink,
        cx: &mut App,
    ) -> Result<Box<dyn RecognitionSession>, SpeechError> {
        let Some(language) = &self.language else {
            return Err(SpeechError::Unsupported);
        };

        let recognizer = WinSpeechRecognizer::Create(language).map_err(speech_error)?;
        let continuous = recognizer
            .ContinuousRecognitionSession()
            .map_err(speech_error)?;
        let (messages, rx) = unbounded();
        let mut session = Session {
            recognizer: recognizer.clone(),
            continuous: continuous.clone(),
            hypothesis_token: None,
            result_token: None,
            completed_token: None,
            messages: messages.clone(),
            _task: None,
        };

        let constraint = SpeechRecognitionTopicConstraint::Create(
            SpeechRecognitionScenario::Dictation,
            &HSTRING::from("dictation"),
        )
        .map_err(speech_error)?;
        recognizer
            .Constraints()
            .and_then(|constraints| constraints.Append(&constraint))
            .map_err(speech_error)?;

        session.hypothesis_token = Some(
            recognizer
                .HypothesisGenerated(&TypedEventHandler::new({
                    let messages = messages.clone();
                    move |_, args: Ref<SpeechRecognitionHypothesisGeneratedEventArgs>| {
                        if let Some(args) = args.as_ref() {
                            send_result(
                                &messages,
                                args.Hypothesis().and_then(|hypothesis| {
                                    hypothesis.Text().map(Message::Hypothesis)
                                }),
                            );
                        }
                        Ok(())
                    }
                }))
                .map_err(speech_error)?,
        );
        session.result_token = Some(
            continuous
                .ResultGenerated(&TypedEventHandler::new({
                    let messages = messages.clone();
                    move |_, args: Ref<SpeechContinuousRecognitionResultGeneratedEventArgs>| {
                        if let Some(args) = args.as_ref() {
                            send_result(&messages, args.Result().map(Message::Result));
                        }
                        Ok(())
                    }
                }))
                .map_err(speech_error)?,
        );
        session.completed_token = Some(
            continuous
                .Completed(&TypedEventHandler::new({
                    let messages = messages.clone();
                    move |_, args: Ref<SpeechContinuousRecognitionCompletedEventArgs>| {
                        if let Some(args) = args.as_ref() {
                            send_result(&messages, args.Status().map(Message::Completed));
                        }
                        Ok(())
                    }
                }))
                .map_err(speech_error)?,
        );

        let separator = self.separator;
        session._task = Some(cx.spawn(async move |cx| {
            let result = dictate(&recognizer, &continuous, rx, &sink, separator, cx).await;
            if let Err(error) = result
                && let Err(error) = cx.update(|cx| sink.error(error, cx))
            {
                tracing::debug!("Application closed before speech error delivery: {error}");
            }
        }));
        Ok(Box::new(session))
    }
}

/// What the WinRT threads and [`Session::finish`] tell the dictation task.
enum Message {
    Hypothesis(HSTRING),
    Result(SpeechRecognitionResult),
    Completed(SpeechRecognitionResultStatus),
    Error(SpeechError),
    /// The user stopped talking.
    Finish,
}

fn send_result(messages: &Sender<Message>, result: windows::core::Result<Message>) {
    let message = result.unwrap_or_else(|error| Message::Error(speech_error(error)));
    if messages.try_send(message).is_err() {
        tracing::debug!("Speech recognition message receiver closed");
    }
}

struct Session {
    recognizer: WinSpeechRecognizer,
    continuous: SpeechContinuousRecognitionSession,
    hypothesis_token: Option<i64>,
    result_token: Option<i64>,
    completed_token: Option<i64>,
    messages: Sender<Message>,
    _task: Option<Task<()>>,
}

impl RecognitionSession for Session {
    /// Ignored: the WinRT recognizer captures from the microphone itself.
    fn push_audio(&mut self, _: &[i16], _: &mut App) {}

    fn finish(&mut self, _: &mut App) {
        // Queued behind the start, so finishing while connecting stops the
        // session as soon as it runs.
        send_result(&self.messages, Ok(Message::Finish));
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(token) = self.hypothesis_token.take() {
            if let Err(error) = self.recognizer.RemoveHypothesisGenerated(token) {
                tracing::warn!("Failed to remove speech hypothesis handler: {error}");
            }
        }
        if let Some(token) = self.result_token.take() {
            if let Err(error) = self.continuous.RemoveResultGenerated(token) {
                tracing::warn!("Failed to remove speech result handler: {error}");
            }
        }
        if let Some(token) = self.completed_token.take() {
            if let Err(error) = self.continuous.RemoveCompleted(token) {
                tracing::warn!("Failed to remove speech completion handler: {error}");
            }
        }

        // Close the recognizer once the cancellation lands, or right away when
        // there is nothing to cancel.
        let recognizer = self.recognizer.clone();
        let closed = self.continuous.CancelAsync().and_then(|action| {
            action.when(move |result| {
                if let Err(error) = result {
                    tracing::debug!("Speech cancellation completed with an error: {error}");
                }
                if let Err(error) = recognizer.Close() {
                    tracing::warn!("Failed to close speech recognizer: {error}");
                }
            })
        });
        if let Err(error) = closed {
            tracing::debug!("Failed to schedule speech cancellation: {error}");
            if let Err(error) = self.recognizer.Close() {
                tracing::warn!("Failed to close speech recognizer: {error}");
            }
        }
    }
}

/// Compile the dictation constraint, start the continuous session and report
/// its results to `sink` until it completes.
async fn dictate(
    recognizer: &WinSpeechRecognizer,
    continuous: &SpeechContinuousRecognitionSession,
    messages: Receiver<Message>,
    sink: &SpeechSink,
    separator: &'static str,
    cx: &mut AsyncApp,
) -> Result<(), SpeechError> {
    let compilation = recognizer
        .CompileConstraintsAsync()
        .map_err(speech_error)?
        .await
        .map_err(speech_error)?;
    let status = compilation.Status().map_err(speech_error)?;
    if status != SpeechRecognitionResultStatus::Success {
        return Err(status_error(status));
    }
    continuous
        .StartAsync()
        .map_err(speech_error)?
        .await
        .map_err(speech_error)?;
    cx.update(|cx| sink.ready(cx))
        .map_err(SpeechError::recognizer)?;

    let mut separator_due = false;
    while let Ok(message) = messages.recv().await {
        match message {
            Message::Hypothesis(text) => {
                let text = joined(separator_due, separator, &text);
                cx.update(|cx| sink.hypothesis(text, cx))
                    .map_err(SpeechError::recognizer)?;
            }
            Message::Result(result) => {
                let Some(text) = phrase_text(&result).map_err(speech_error)? else {
                    continue;
                };
                let text = joined(separator_due, separator, &text);
                separator_due = true;
                cx.update(|cx| sink.phrase(text, cx))
                    .map_err(SpeechError::recognizer)?;
            }
            Message::Completed(status) => {
                return match status {
                    // Stopped, cancelled, or ended by the silence timeout.
                    SpeechRecognitionResultStatus::Success
                    | SpeechRecognitionResultStatus::UserCanceled
                    | SpeechRecognitionResultStatus::TimeoutExceeded => {
                        cx.update(|cx| sink.finish(cx))
                            .map_err(SpeechError::recognizer)?;
                        Ok(())
                    }
                    status => Err(status_error(status)),
                };
            }
            Message::Error(error) => return Err(error),
            // Stopping flushes the last phrase, then completes the session.
            Message::Finish => {
                let stopped = match continuous.StopAsync() {
                    Ok(stop) => stop.await,
                    Err(error) => Err(error),
                };
                // The session may have ended by itself, e.g. after the silence
                // timeout, with its `Completed` still queued behind this; stopping
                // it then fails, and that queued event ends the session instead.
                if let Err(error) = stopped {
                    log::debug!("speech: stopping dictation failed: {error}");
                }
            }
        }
    }
    Ok(())
}

/// The text of a recognized phrase, unless it was rejected or empty.
fn phrase_text(result: &SpeechRecognitionResult) -> windows::core::Result<Option<HSTRING>> {
    if result.Status()? != SpeechRecognitionResultStatus::Success
        || result.Confidence()? == SpeechRecognitionConfidence::Rejected
    {
        return Ok(None);
    }
    Ok(Some(result.Text()?).filter(|text| !text.is_empty()))
}

/// `text` preceded by `separator` once a phrase has been committed.
fn joined(separator_due: bool, separator: &str, text: &HSTRING) -> SharedString {
    let text = text.to_string_lossy();
    if separator_due {
        format!("{separator}{text}").into()
    } else {
        text.into()
    }
}

/// What goes between two phrases, which dictation returns without surrounding
/// whitespace: a space, except in languages written without spaces between
/// words (Chinese, Japanese, Thai, Lao, Khmer, Burmese).
fn phrase_separator(tag: &str) -> &'static str {
    let primary = tag.split('-').next().unwrap_or_default();
    let unspaced = ["zh", "yue", "ja", "th", "lo", "km", "my"];
    if unspaced
        .iter()
        .any(|lang| primary.eq_ignore_ascii_case(lang))
    {
        ""
    } else {
        " "
    }
}

/// The language to dictate `locale` (or, without one, the system's speech
/// language) in, if a speech pack supports it.
///
/// A bare language such as `en` falls back to the first supported region.
fn supported_language(locale: Option<&str>) -> windows::core::Result<Option<Language>> {
    let requested = match locale {
        Some(locale) => {
            let tag = HSTRING::from(locale);
            if !Language::IsWellFormed(&tag)? {
                return Ok(None);
            }
            Language::CreateLanguage(&tag)?
        }
        None => WinSpeechRecognizer::SystemSpeechLanguage()?,
    };
    let requested = requested.LanguageTag()?.to_string_lossy();
    let region_prefix = format!("{requested}-");

    let mut fallback = None;
    for language in WinSpeechRecognizer::SupportedTopicLanguages()? {
        let tag = language.LanguageTag()?.to_string_lossy();
        if tag.eq_ignore_ascii_case(&requested) {
            return Ok(Some(language));
        }
        if fallback.is_none()
            && tag.len() > region_prefix.len()
            && tag[..region_prefix.len()].eq_ignore_ascii_case(&region_prefix)
        {
            fallback = Some(language);
        }
    }
    Ok(fallback)
}

fn speech_error(error: windows::core::Error) -> SpeechError {
    match error.code() {
        E_ACCESSDENIED => SpeechError::PermissionDenied,
        NO_CAPTURE_DEVICES => SpeechError::NoInputDevice,
        PRIVACY_POLICY_NOT_ACCEPTED => SpeechError::recognizer(anyhow!(
            "Online speech recognition is turned off; turn it on in Settings > \
             Privacy & security > Speech"
        )),
        _ => SpeechError::recognizer(error),
    }
}

fn status_error(status: SpeechRecognitionResultStatus) -> SpeechError {
    match status {
        SpeechRecognitionResultStatus::TopicLanguageNotSupported => SpeechError::Unsupported,
        SpeechRecognitionResultStatus::MicrophoneUnavailable => SpeechError::NoInputDevice,
        SpeechRecognitionResultStatus::NetworkFailure => {
            SpeechError::recognizer(anyhow!("could not reach the online speech service"))
        }
        SpeechRecognitionResultStatus::AudioQualityFailure => {
            SpeechError::recognizer(anyhow!("the audio was too poor to recognize"))
        }
        status => SpeechError::recognizer(anyhow!("recognition ended with status {}", status.0)),
    }
}
