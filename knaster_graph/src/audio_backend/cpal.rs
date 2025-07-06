//! # CPAL audio backend
//!
//! CPAL is the default audio backend, supporting all major platforms. However, it doesn't support
//! duplex audio streams (audio input and output in the same callback). If you want to process
//! incoming audio, use a different backend.
use core::marker::PhantomData;

use crate::audio_backend::{AudioBackend, AudioBackendError};
use crate::processor::AudioProcessor;
#[cfg(all(debug_assertions, feature = "assert_no_alloc"))]
use assert_no_alloc::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use knaster_core::{Block, Float};
/// no_std_compat prelude import, supporting both std and no_std
use std::prelude::v1::*;

#[allow(missing_docs)]
pub struct CpalBackendOptions {
    pub device: String,
    pub verbose: bool,
}
impl Default for CpalBackendOptions {
    fn default() -> Self {
        Self {
            device: "default".into(),
            verbose: false,
        }
    }
}
/// CPAL backend for convenience. The CPAL backend currently does not support passing on audio inputs from outside the program.
pub struct CpalBackend<F> {
    stream: Option<cpal::Stream>,
    sample_rate: u32,
    config: cpal::SupportedStreamConfig,
    device: cpal::Device,
    _marker: PhantomData<F>,
}

/// # Safety
/// CPAL streams aren't Send or Sync. The reasons for this are vague, mentioning that the Android AAudio API "prohibits calling certain
/// functions within the callback". I think the CpalBackend encapsulation is adequate.
///
/// More info:
/// https://github.com/RustAudio/cpal/blob/582e93c41d6073df5d5da871989c5fd581ea04b8/src/platform/mod.rs
unsafe impl<F> Send for CpalBackend<F> {}
unsafe impl<F> Sync for CpalBackend<F> {}

impl<F> CpalBackend<F> {
    /// Create a new CpalBackend using the default host, getting a device, but not a stream.
    pub fn new(options: CpalBackendOptions) -> Result<Self, AudioBackendError> {
        let host = cpal::default_host();

        let device = if options.device == "default" {
            host.default_output_device()
        } else {
            host.output_devices()?
                .find(|x| x.name().map(|y| y == options.device).unwrap_or(false))
        }
        .expect("failed to find output device");
        if options.verbose {
            log::info!("Output device: {}", device.name()?);
        }

        let config = device.default_output_config().unwrap();
        if options.verbose {
            log::info!("Default output config: {:?}", config);
        }
        Ok(Self {
            stream: None,
            sample_rate: config.sample_rate().0 as u32,
            config,
            device,
            _marker: PhantomData,
        })
    }
    /// The number of outputs for the device's default output config
    pub fn num_outputs(&self) -> usize {
        self.config.channels() as usize
    }
    /// Leak the backend, thereby avoiding running `Drop` and closing the audio stream.
    pub fn leak(self) {
        Box::leak(Box::new(self));
    }
}

impl<F: Float> AudioBackend for CpalBackend<F> {
    type Sample = F;
    fn start_processing(
        &mut self,
        runner: AudioProcessor<Self::Sample>,
    ) -> Result<(), AudioBackendError> {
        if self.stream.is_some() {
            return Err(AudioBackendError::BackendAlreadyRunning);
        }
        if runner.outputs() != self.config.channels() {
            panic!(
                "CpalBackend expects a graph with the same number of outputs as the device. Check CpalBackend::channels()."
            )
        }
        if runner.inputs() > 0 {
            log::error!(
                "Warning: CpalBackend currently does not support inputs into the top level Graph. Top level graph inputs will have no data."
            );
        }
        let config = self.config.clone();
        let stream = match self.config.sample_format() {
            cpal::SampleFormat::I16 => run::<i16, F>(&self.device, &config.into(), runner),
            cpal::SampleFormat::U16 => run::<u16, F>(&self.device, &config.into(), runner),
            cpal::SampleFormat::I8 => run::<i8, F>(&self.device, &config.into(), runner),
            cpal::SampleFormat::I32 => run::<i32, F>(&self.device, &config.into(), runner),
            cpal::SampleFormat::I64 => run::<i64, F>(&self.device, &config.into(), runner),
            cpal::SampleFormat::U8 => run::<u8, F>(&self.device, &config.into(), runner),
            cpal::SampleFormat::U32 => run::<u32, F>(&self.device, &config.into(), runner),
            cpal::SampleFormat::U64 => run::<u64, F>(&self.device, &config.into(), runner),
            cpal::SampleFormat::F32 => run::<f32, F>(&self.device, &config.into(), runner),
            cpal::SampleFormat::F64 => run::<f64, F>(&self.device, &config.into(), runner),
            _ => todo!(),
        }?;
        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioBackendError> {
        // Drop the stream to stop it
        self.stream.take();
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn block_size(&self) -> Option<usize> {
        None
    }

    fn native_output_channels(&self) -> Option<usize> {
        Some(self.num_outputs())
    }

    fn native_input_channels(&self) -> Option<usize> {
        // TODO: support duplex streams
        Some(0)
    }
}

fn run<T, F: Float>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut audio_processor: AudioProcessor<F>,
) -> Result<cpal::Stream, AudioBackendError>
where
    T: cpal::Sample + cpal::FromSample<f32> + cpal::SizedSample + crate::core::fmt::Display,
{
    let channels = config.channels as usize;

    // TODO: Send error via ArLogSender instead.
    let err_fn = |err| log::error!("CPAL error: an error occurred on stream: {}", err);

    let graph_block_size = audio_processor.block_size();
    let mut sample_counter = graph_block_size; // Immediately process a block
    let stream = device.build_output_stream(
        config,
        move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
            // TODO: When CPAL support duplex streams, copy inputs to graph inputs here.
            #[cfg(all(debug_assertions, feature = "assert_no_alloc"))]
            {
                assert_no_alloc(|| {
                    for frame in output.chunks_mut(channels) {
                        if sample_counter >= graph_block_size {
                            // CPAL currently does not support duplex streams
                            unsafe { audio_processor.run(&[]) };
                            sample_counter = 0;
                        }
                        let out_block = audio_processor.output_block();
                        for (channel_i, out) in frame.iter_mut().enumerate() {
                            let sample = out_block.read(channel_i, sample_counter);
                            let value: T = T::from_sample(sample.to_f32());
                            *out = value;
                        }
                        sample_counter += 1;
                    }
                })
            }
            #[cfg(not(all(debug_assertions, feature = "assert_no_alloc")))]
            {
                for frame in output.chunks_mut(channels) {
                    if sample_counter >= graph_block_size {
                        // CPAL currently does not support duplex streams
                        unsafe { audio_processor.run(&[]) };
                        sample_counter = 0;
                    }
                    let out_block = audio_processor.output_block();
                    // println!("{}", T::from_sample(buffer.read(0, sample_counter)));
                    for (channel_i, out) in frame.iter_mut().enumerate() {
                        let sample = out_block.read(channel_i, sample_counter);
                        let value: T = T::from_sample(sample.to_f32());
                        *out = value;
                    }
                    sample_counter += 1;
                }
            }
        },
        err_fn,
        None,
    )?;

    stream.play()?;
    Ok(stream)
}
