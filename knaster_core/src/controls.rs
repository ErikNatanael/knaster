//! Control (float parameter) metadata

use knaster_primitives::{ControlPurpose, ControlRange, Float};
use thiserror::Error;

/// Error related to knaster parameters
#[derive(Debug, Clone, Error)]
pub enum ControlError {
    #[error("Control name `{0}` not found")]
    #[allow(missing_docs)]
    NameNotFound(heapless::String<16>),
    #[error("The control index is out of bounds.")]
    #[allow(missing_docs)]
    IndexOutOfBounds,
    #[error(
        "The graph within which the node you are trying to set controls for does not exist anymore."
    )]
    #[allow(missing_docs)]
    GraphWasFreed,
}

impl ControlError {
    /// Create a ControlError::NameNotFound
    pub fn name_not_found(name: &str) -> Self {
        let name = truncate(name, 16);
        let name =
            heapless::String::try_from(name).expect("name should fit because we have truncated it");
        Self::NameNotFound(name)
    }
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}
/// Metadata about a control.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlMetadata<F: Float> {
    name: heapless::String<16>,
    default: F,
    range: Option<ControlRange<F>>,
    purpose: Option<ControlPurpose>,
}

impl<F: Float> Default for ControlMetadata<F> {
    fn default() -> Self {
        Self {
            name: Default::default(),
            default: Default::default(),
            range: None,
            purpose: None,
        }
    }
}

impl<F: Float> ControlMetadata<F> {
    /// Create a new ControlMetadata with the given name, default value and range.
    ///
    /// # Panics
    /// Panics if the name is longer than 16 characters.
    pub fn new(name: impl AsRef<str>, default: F, range: ControlRange<F>) -> Self {
        Self {
            name: name
                .as_ref()
                .try_into()
                .expect("Control names above 16 characters are not supported"),
            default,
            range: Some(range),
            purpose: None,
        }
    }
    /// Get the purpose of this control, if any. This is a hint for GUIs and similar.
    pub fn with_purpose(mut self, purpose: ControlPurpose) -> Self {
        self.purpose = Some(purpose);
        self
    }
    /// Get the name of this control.
    pub fn name(&self) -> &heapless::String<16> {
        &self.name
    }
    /// Get the name of this control as a `&str`.
    pub fn name_str(&self) -> &str {
        self.name.as_str()
    }
    /// Get the range of this control, if any. This is a hint for GUIs and similar.
    pub fn range(&self) -> &Option<ControlRange<F>> {
        &self.range
    }
    /// Get the purpose of this control, if any. This is a hint for GUIs and similar.
    pub fn purpose(&self) -> &Option<ControlPurpose> {
        &self.purpose
    }
    /// Get the default value of this control
    pub fn default(&self) -> F {
        self.default
    }
}
