use crate::audio_backend::{AudioBackend, AudioBackendError};
use crate::runner::Runner;
#[cfg(all(debug_assertions, feature = "assert_no_alloc"))]
use assert_no_alloc::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use knaster_core::{Block, Float};

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
pub struct CpalBackend {
    stream: Option<cpal::Stream>,
    sample_rate: u32,
    config: cpal::SupportedStreamConfig,
    device: cpal::Device,
}

impl CpalBackend {
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
            println!("Output device: {}", device.name()?);
        }

        let config = device.default_output_config().unwrap();
        if options.verbose {
            println!("Default output config: {:?}", config);
        }
        Ok(Self {
            stream: None,
            sample_rate: config.sample_rate().0 as u32,
            config,
            device,
        })
    }
    /// The number of outputs for the device's default output config
    pub fn num_outputs(&self) -> usize {
        self.config.channels() as usize
    }
}

impl AudioBackend for CpalBackend {
    fn start_processing<F: Float>(&mut self, runner: Runner<F>) -> Result<(), AudioBackendError> {
        if self.stream.is_some() {
            return Err(AudioBackendError::BackendAlreadyRunning);
        }
        if runner.outputs() != self.config.channels() as usize {
            panic!("CpalBackend expects a graph with the same number of outputs as the device. Check CpalBackend::channels().")
        }
        if runner.inputs() > 0 {
            eprintln!(
                "Warning: CpalBackend currently does not support inputs into the top level Graph."
            )
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
        self.stream = Some(stream);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioBackendError> {
        todo!()
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
    mut runner: Runner<F>,
) -> Result<cpal::Stream, AudioBackendError>
where
    T: cpal::Sample + cpal::FromSample<f32> + cpal::SizedSample + std::fmt::Display,
{
    let channels = config.channels as usize;

    // TODO: Send error back from the audio thread in a unified way.
    let err_fn = |err| eprintln!("CPAL error: an error occurred on stream: {}", err);

    let graph_block_size = runner.block_size();
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
                            unsafe { runner.run(&[]) };
                            sample_counter = 0;
                        }
                        let out_block = runner.output_block();
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
                        unsafe { runner.run(&[]) };
                        sample_counter = 0;
                    }
                    let out_block = runner.output_block();
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
