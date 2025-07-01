//! # Audio backends
//!
//! [`AudioBackend`] is the unified API for different audio backends. Because audio backends often
//! have wildly different APIs, each backend also has backend specific functionality.
//!
//! Currently, the following backends are supported, each requiring the corresponding feature:
//!
//! - [`cpal`](https://github.com/RustAudio/cpal)
//! - [`jack`](https://github.com/RustAudio/rust-jack)
#[cfg(feature = "cpal")]
pub mod cpal;
#[cfg(feature = "jack")]
pub mod jack;

use alloc::string::String;

use knaster_core::Float;

use crate::processor::AudioProcessor;

/// Unified API for different backends.
pub trait AudioBackend {
    /// Float type (f32 or f64) for this audio backend.
    type Sample: Float;
    /// Starts processing and returns a [`Controller`]. This is the easiest
    /// option and will run the [`Controller`] in a loop on a new thread.
    fn start_processing(&mut self, audio_processor: AudioProcessor<Self::Sample>) -> Result<(), AudioBackendError>;
    /// Stop the backend
    fn stop(&mut self) -> Result<(), AudioBackendError>;
    /// Get the native sample rate of the backend
    fn sample_rate(&self) -> u32;
    /// Get the native block size of the backend if there is one
    fn block_size(&self) -> Option<usize>;
    /// Get the native number of output channels for this backend, if any
    fn native_output_channels(&self) -> Option<usize>;
    /// Get the native number of input channels for this backend, if any
    fn native_input_channels(&self) -> Option<usize>;
}

#[allow(missing_docs)]
#[derive(thiserror::Error, Debug)]
pub enum AudioBackendError {
    #[error(
        "You tried to start a backend that was already running. A backend can only be started once."
    )]
    BackendAlreadyRunning,
    #[error("You tried to stop a backend that was already stopped.")]
    BackendNotRunning,
    #[error("Unable to create a node from the Graph: {0}")]
    CouldNotCreateNode(String),
    #[cfg(feature = "jack")]
    #[error(transparent)]
    JackError(#[from] ::jack::Error),
    #[cfg(feature = "cpal")]
    #[error(transparent)]
    CpalDevicesError(#[from] ::cpal::DevicesError),
    #[cfg(feature = "cpal")]
    #[error(transparent)]
    CpalDeviceNameError(#[from] ::cpal::DeviceNameError),
    #[cfg(feature = "cpal")]
    #[error(transparent)]
    CpalStreamError(#[from] ::cpal::StreamError),
    #[cfg(feature = "cpal")]
    #[error(transparent)]
    CpalBuildStreamError(#[from] ::cpal::BuildStreamError),
    #[cfg(feature = "cpal")]
    #[error(transparent)]
    CpalPlayStreamError(#[from] ::cpal::PlayStreamError),
}
