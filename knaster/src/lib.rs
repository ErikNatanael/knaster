#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

// Switches between std and core based on features. This reduces boilerplate when importing.
mod core {
    #[cfg(not(feature = "std"))]
    pub use alloc::*;
    #[cfg(not(feature = "std"))]
    pub use core::*;
    #[cfg(feature = "std")]
    pub use std::*;
}

pub mod math_ugens;
pub mod prelude;
pub mod preludef32;
pub(crate) mod subprelude_fundamental_types;
use core::boxed::Box;

pub use knaster_graph::*;
use knaster_graph::{
    audio_backend::{AudioBackend, AudioBackendError, cpal::CpalBackendOptions},
    graph::{Graph, GraphOptions},
    log::ArLogMessage,
    runner::{Runner, RunnerOptions},
    typenum::{U0, U2},
};

pub struct KnasterBuilder<F: Float> {
    runner_options: RunnerOptions,
    graph_options: GraphOptions,
    backend: Option<Box<dyn AudioBackend<Sample = F>>>,
    log_handler: Option<Box<dyn FnMut(&[ArLogMessage]) + Send>>,
}
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
        let (top_level_graph, runner, mut log_receiver) =
            Runner::<F>::new::<U0, U2>(self.runner_options);
        std::thread::spawn(move || {
            loop {
                log_receiver.recv(&mut log_handler);
                std::thread::sleep(core::time::Duration::from_secs_f32(0.1))
            }
        });
        backend.start_processing(runner)?;
        Box::leak(backend);
        Ok(top_level_graph)
    }
    pub fn backend(mut self, backend: impl AudioBackend<Sample = F> + 'static) -> Self {
        self.runner_options.block_size = backend.block_size().unwrap_or(64);
        self.runner_options.sample_rate = backend.sample_rate();
        self.backend = Some(Box::new(backend));
        self
    }
    pub fn log_handler(mut self, log_handler: Box<dyn FnMut(&[ArLogMessage]) + Send>) -> Self {
        self.log_handler = Some(log_handler);
        self
    }
}
pub fn knaster<F: Float>() -> KnasterBuilder<F> {
    KnasterBuilder {
        runner_options: RunnerOptions::default(),
        graph_options: GraphOptions::default(),
        backend: None,
        log_handler: None,
    }
}
