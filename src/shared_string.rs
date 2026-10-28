use std::{
    borrow::{Borrow, Cow},
    iter,
    sync::Arc,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// A shared string is an immutable string that can be cheaply cloned in WGPUI
/// tasks. Essentially an abstraction over an `Arc<str>` and `&'static str`,
/// currently backed by a [`SmolStr`].
#[derive(Eq, PartialEq, PartialOrd, Ord, Hash, Clone)]
pub struct SharedString(SmolStr);

impl std::ops::Deref for SharedString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl SharedString {
    /// Creates a static [`SharedString`] from a `&'static str`.
    pub const fn new_static(str: &'static str) -> Self {
        Self(SmolStr::new_static(str))
    }

    /// Creates a [`SharedString`].
    pub fn new(str: impl AsRef<str>) -> Self {
        SharedString(SmolStr::new(str))
    }

    /// Get a &str from the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl JsonSchema for SharedString {
    fn inline_schema() -> bool {
        String::inline_schema()
    }

    fn schema_name() -> Cow<'static, str> {
        String::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        String::json_schema(generator)
    }
}

impl Default for SharedString {
    fn default() -> Self {
        Self::new_static("")
    }
}

impl AsRef<str> for SharedString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for SharedString {
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}

impl std::fmt::Debug for SharedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Display for SharedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}

impl PartialEq<String> for SharedString {
    fn eq(&self, other: &String) -> bool {
        self.as_ref() == other
    }
}

impl PartialEq<SharedString> for String {
    fn eq(&self, other: &SharedString) -> bool {
        self == other.as_ref()
    }
}

impl PartialEq<str> for SharedString {
    fn eq(&self, other: &str) -> bool {
        self.as_ref() == other
    }
}

impl<'a> PartialEq<&'a str> for SharedString {
    fn eq(&self, other: &&'a str) -> bool {
        self.as_ref() == *other
    }
}

impl From<&SharedString> for SharedString {
    #[inline]
    fn from(s: &SharedString) -> SharedString {
        s.clone()
    }
}

impl From<&str> for SharedString {
    #[inline]
    fn from(s: &str) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<char> for SharedString {
    #[inline]
    fn from(c: char) -> SharedString {
        SharedString(SmolStr::from_iter(iter::once(c)))
    }
}

impl From<&mut str> for SharedString {
    #[inline]
    fn from(s: &mut str) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<&String> for SharedString {
    #[inline]
    fn from(s: &String) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<String> for SharedString {
    #[inline(always)]
    fn from(text: String) -> Self {
        SharedString(SmolStr::from(text))
    }
}

impl From<Box<str>> for SharedString {
    #[inline]
    fn from(s: Box<str>) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<Arc<str>> for SharedString {
    #[inline]
    fn from(s: Arc<str>) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<&Arc<str>> for SharedString {
    #[inline]
    fn from(s: &Arc<str>) -> SharedString {
        SharedString(SmolStr::from(s.clone()))
    }
}

impl<'a> From<Cow<'a, str>> for SharedString {
    #[inline]
    fn from(s: Cow<'a, str>) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<SharedString> for Arc<str> {
    #[inline(always)]
    fn from(text: SharedString) -> Self {
        text.0.into()
    }
}

impl From<SharedString> for String {
    #[inline(always)]
    fn from(text: SharedString) -> Self {
        text.0.into()
    }
}

impl Serialize for SharedString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}

impl<'de> Deserialize<'de> for SharedString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        SmolStr::deserialize(deserializer).map(SharedString)
    }
}

impl<'a> From<util::arc_cow::ArcCow<'a, str>> for SharedString {
    fn from(value: util::arc_cow::ArcCow<'a, str>) -> Self {
        match value {
            util::arc_cow::ArcCow::Borrowed(value) => Self::from(value),
            util::arc_cow::ArcCow::Owned(value) => Self::from(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_runtime_text_is_owned_independently_of_its_source() {
        let value = {
            let source = format!("Borrowed {} {}", "Δ", "🦀");
            SharedString::from(source.as_str())
        };
        assert_eq!(value.as_str(), "Borrowed Δ 🦀");
        let optional = Some(value);
        let borrowed: Option<&str> = optional.as_deref();
        assert_eq!(borrowed, Some("Borrowed Δ 🦀"));
    }

    #[test]
    fn supported_string_forms_preserve_unicode() {
        let mut text = "Δ🦀".to_owned();
        let boxed: Box<str> = text.clone().into_boxed_str();
        let shared: Arc<str> = Arc::from(text.as_str());
        let values = [
            SharedString::from(text.as_str()),
            SharedString::from(text.as_mut_str()),
            SharedString::from(&text),
            SharedString::from(text.clone()),
            SharedString::from(boxed),
            SharedString::from(shared.clone()),
            SharedString::from(&shared),
            SharedString::from(Cow::Borrowed(text.as_str())),
            SharedString::from(Cow::Owned(text.clone())),
            SharedString::new(text.as_str()),
        ];
        for value in values {
            assert_eq!(value, "Δ🦀");
        }
        assert_eq!(SharedString::from('🦀'), "🦀");
    }

    #[test]
    fn explicit_static_storage_and_legacy_arc_cow_are_preserved() {
        const TEXT: &str = "A static string long enough to exceed inline storage.";
        let value = SharedString::new_static(TEXT);
        assert_eq!(value.as_ptr(), TEXT.as_ptr());
        assert_eq!(
            SharedString::from(util::arc_cow::ArcCow::Borrowed(TEXT)),
            TEXT
        );
        let source = String::from("runtime borrowed legacy value");
        assert_eq!(
            SharedString::from(util::arc_cow::ArcCow::Borrowed(source.as_str())),
            source
        );
    }

    #[test]
    fn serialization_keeps_the_plain_string_contract() -> anyhow::Result<()> {
        let value = SharedString::from("escaped \" text Δ🦀");
        let encoded = serde_json::to_string(&value)?;
        let decoded: SharedString = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, value);
        assert!(serde_json::from_str::<SharedString>("null").is_err());
        Ok(())
    }
}
