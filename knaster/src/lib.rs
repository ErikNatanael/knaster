#![no_std]

// Switches between std and core based on features. This reduces boilerplate when importing.
extern crate no_std_compat as std;

// Switches between std and core based on features. This reduces boilerplate when importing.
mod core {
    pub use no_std_compat::*;
}

pub mod math_ugens;
pub mod prelude;
pub mod preludef32;
pub(crate) mod subprelude_fundamental_types;
use core::boxed::Box;

pub use knaster_graph::*;
use knaster_graph::{
    audio_backend::{AudioBackend, AudioBackendError},
    graph::Graph,
    log::ArLogMessage,
    processor::{AudioProcessor, AudioProcessorOptions},
    typenum::{U0, U2},
};

#[cfg(feature = "cpal")]
use knaster_graph::audio_backend::cpal::CpalBackendOptions;

#[cfg(feature = "cpal")]
#[allow(clippy::type_complexity)]
pub struct KnasterBuilder<F: Float> {
    runner_options: AudioProcessorOptions,
    backend: Option<Box<dyn AudioBackend<Sample = F>>>,
    log_handler: Option<Box<dyn FnMut(&[ArLogMessage]) + Send>>,
}
#[cfg(feature = "cpal")]
impl<F: Float> KnasterBuilder<F> {
    pub fn start(self) -> Result<Graph<F>, AudioBackendError> {
        let mut backend = if let Some(backend) = self.backend {
            backend
        } else {
            Box::new(knaster_graph::audio_backend::cpal::CpalBackend::new(
                CpalBackendOptions::default(),
            )?)
        };
        let mut log_handler = self.log_handler.unwrap_or_else(|| {
            Box::new(|m| {
                ::log::info!("{:?}", m);
            })
        });

        // Create a graph
        let (top_level_graph, audio_processor, mut log_receiver) =
            AudioProcessor::<F>::new::<U0, U2>(self.runner_options);
        std::thread::spawn(move || {
            loop {
                log_receiver.recv(&mut log_handler);
                std::thread::sleep(core::time::Duration::from_secs_f32(0.1))
            }
        });
        backend.start_processing(audio_processor)?;
        Box::leak(backend);
        Ok(top_level_graph)
    }
    pub fn backend(mut self, backend: impl AudioBackend<Sample = F> + 'static) -> Self {
        self.runner_options.block_size = backend.block_size().unwrap_or(64);
        self.runner_options.sample_rate = backend.sample_rate();
        self.backend = Some(Box::new(backend));
        self
    }
    #[allow(clippy::type_complexity)]
    pub fn log_handler(mut self, log_handler: Box<dyn FnMut(&[ArLogMessage]) + Send>) -> Self {
        self.log_handler = Some(log_handler);
        self
    }
}

#[cfg(feature = "cpal")]
pub fn knaster<F: Float>() -> KnasterBuilder<F> {
    KnasterBuilder {
        runner_options: AudioProcessorOptions::default(),
        backend: None,
        log_handler: None,
    }
}
