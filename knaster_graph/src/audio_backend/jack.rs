use crate::core::vec::Vec;

use crate::core::format;

use crate::core::vec;

use crate::audio_backend::{AudioBackend, AudioBackendError};
use crate::runner::Runner;
#[cfg(all(debug_assertions, feature = "assert_no_alloc"))]
use assert_no_alloc::*;
use knaster_core::Block;
use knaster_core::Float;
use knaster_core::VecBlock;
enum JackClient<F: Float> {
    Passive(jack::Client),
    Active(jack::AsyncClient<JackNotifications, JackProcess<F>>),
}

/// A backend using JACK
pub struct JackBackend<F: Float> {
    client: Option<JackClient<F>>,
    sample_rate: u32,
    block_size: usize,
}

impl<F: Float> JackBackend<F> {
    /// Create a new JACK client using the given name
    pub fn new<S: AsRef<str>>(name: S) -> Result<Self, jack::Error> {
        // Create client
        let (client, _status) =
            jack::Client::new(name.as_ref(), jack::ClientOptions::NO_START_SERVER).unwrap();
        let sample_rate = client.sample_rate() as u32;
        let block_size = client.buffer_size() as usize;
        Ok(Self {
            client: Some(JackClient::Passive(client)),
            sample_rate,
            block_size,
        })
    }
}

impl<F: Float> AudioBackend for JackBackend<F> {
    type Sample = F;
    fn stop(&mut self) -> Result<(), AudioBackendError> {
        match self.client.take() {
            Some(JackClient::Active(active_client)) => {
                active_client.deactivate().unwrap();
                Ok(())
            }
            _ => Err(AudioBackendError::BackendNotRunning),
        }
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn block_size(&self) -> Option<usize> {
        Some(self.block_size)
    }

    fn native_output_channels(&self) -> Option<usize> {
        None
    }

    fn native_input_channels(&self) -> Option<usize> {
        None
    }

    fn start_processing(
        &mut self,
        runner: crate::runner::Runner<Self::Sample>,
    ) -> Result<(), AudioBackendError> {
        match self.client.take() {
            Some(JackClient::Passive(client)) => {
                let mut in_ports = vec![];
                let mut out_ports = vec![];
                let num_inputs = runner.inputs();
                let num_outputs = runner.outputs();
                for i in 0..num_inputs {
                    in_ports
                        .push(client.register_port(&format!("in_{i}"), jack::AudioIn::default())?);
                }
                for i in 0..num_outputs {
                    out_ports.push(
                        client.register_port(&format!("out_{i}"), jack::AudioOut::default())?,
                    );
                }
                let input_block = VecBlock::new(num_inputs as usize, self.block_size);
                let mut input_block_pointers = Vec::with_capacity(num_inputs as usize);
                for i in 0..num_inputs {
                    input_block_pointers.push(input_block.channel_as_slice(i as usize).as_ptr());
                }
                let jack_process = JackProcess {
                    runner,
                    in_ports,
                    out_ports,
                    input_block,
                    input_block_pointers,
                };
                // Activate the client, which starts the processing.
                let active_client = client
                    .activate_async(JackNotifications, jack_process)
                    .unwrap();
                self.client = Some(JackClient::Active(active_client));
                Ok(())
            }
            _ => Err(AudioBackendError::BackendAlreadyRunning),
        }
    }
}

struct JackProcess<F: Float> {
    runner: Runner<F>,
    in_ports: Vec<jack::Port<jack::AudioIn>>,
    out_ports: Vec<jack::Port<jack::AudioOut>>,
    input_block: VecBlock<F>,
    input_block_pointers: Vec<*const F>,
}
unsafe impl<F: Float> Send for JackProcess<F> {}
unsafe impl<F: Float> Sync for JackProcess<F> {}

impl<F: Float> jack::ProcessHandler for JackProcess<F> {
    fn process(&mut self, _: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        // Duplication due to conditional compilation
        #[cfg(all(debug_assertions, feature = "assert_no_alloc"))]
        {
            assert_no_alloc(|| {
                let graph_input_buffers = self.run_graph.graph_input_buffers();
                for (i, in_port) in self.in_ports.iter().enumerate() {
                    let in_port_slice = in_port.as_slice(ps);
                    let in_buffer = unsafe { graph_input_buffers.get_channel_mut(i) };
                    // in_buffer.clone_from_slice(in_port_slice);
                    for (from_jack, graph_in) in in_port_slice.iter().zip(in_buffer.iter_mut()) {
                        *graph_in = *from_jack as Sample;
                    }
                }
                self.run_graph.run_resources_communication(50);
                self.run_graph.process_block();

                let graph_output_buffers = self.run_graph.graph_output_buffers_mut();
                for (i, out_port) in self.out_ports.iter_mut().enumerate() {
                    let out_buffer = unsafe { graph_output_buffers.get_channel_mut(i) };
                    for sample in out_buffer.iter_mut() {
                        *sample = sample.clamp(-1.0, 1.0);
                        if sample.is_nan() {
                            *sample = 0.0;
                        }
                    }
                    let out_port_slice = out_port.as_mut_slice(ps);
                    // out_port_slice.clone_from_slice(out_buffer);
                    for (to_jack, graph_out) in out_port_slice.iter_mut().zip(out_buffer.iter()) {
                        *to_jack = *graph_out as f32;
                    }
                }
                jack::Control::Continue
            })
        }
        #[cfg(not(all(debug_assertions, feature = "assert_no_alloc")))]
        {
            for (i, in_port) in self.in_ports.iter().enumerate() {
                let in_port_slice = in_port.as_slice(ps);
                let in_buffer = self.input_block.channel_as_slice_mut(i);
                // in_buffer.clone_from_slice(in_port_slice);
                for (from_jack, graph_in) in in_port_slice.iter().zip(in_buffer.iter_mut()) {
                    *graph_in = F::new(*from_jack);
                }
            }
            unsafe { self.runner.run(&self.input_block_pointers) }

            let graph_output_buffers = self.runner.output_block();
            for (i, out_port) in self.out_ports.iter_mut().enumerate() {
                let out_buffer = graph_output_buffers.channel_as_slice_mut(i);
                for sample in out_buffer.iter_mut() {
                    *sample = sample.clamp(-F::ONE, F::ONE);
                    if sample.is_nan() {
                        *sample = F::ZERO;
                    }
                }
                let out_port_slice = out_port.as_mut_slice(ps);
                // out_port_slice.clone_from_slice(out_buffer);
                for (to_jack, graph_out) in out_port_slice.iter_mut().zip(out_buffer.iter()) {
                    *to_jack = graph_out.to_f32().unwrap();
                }
            }
            jack::Control::Continue
        }
    }
}

struct JackNotifications;
impl Default for JackNotifications {
    fn default() -> Self {
        Self
    }
}

impl jack::NotificationHandler for JackNotifications {
    fn thread_init(&self, _: &jack::Client) {}

    unsafe fn shutdown(&mut self, _status: jack::ClientStatus, _reason: &str) {}

    fn freewheel(&mut self, _: &jack::Client, _is_enabled: bool) {}

    fn sample_rate(&mut self, _: &jack::Client, _srate: jack::Frames) -> jack::Control {
        // println!("JACK: sample rate changed to {}", srate);
        jack::Control::Continue
    }

    fn client_registration(&mut self, _: &jack::Client, _name: &str, _is_reg: bool) {
        // println!(
        //     "JACK: {} client with name \"{}\"",
        //     if is_reg { "registered" } else { "unregistered" },
        //     name
        // );
    }

    fn port_registration(&mut self, _: &jack::Client, _port_id: jack::PortId, _is_reg: bool) {
        // println!(
        //     "JACK: {} port with id {}",
        //     if is_reg { "registered" } else { "unregistered" },
        //     port_id
        // );
    }

    fn port_rename(
        &mut self,
        _: &jack::Client,
        _port_id: jack::PortId,
        _old_name: &str,
        _new_name: &str,
    ) -> jack::Control {
        // println!(
        //     "JACK: port with id {} renamed from {} to {}",
        //     port_id, old_name, new_name
        // );
        jack::Control::Continue
    }

    fn ports_connected(
        &mut self,
        _: &jack::Client,
        _port_id_a: jack::PortId,
        _port_id_b: jack::PortId,
        _are_connected: bool,
    ) {
        // println!(
        //     "JACK: ports with id {} and {} are {}",
        //     port_id_a,
        //     port_id_b,
        //     if are_connected {
        //         "connected"
        //     } else {
        //         "disconnected"
        //     }
        // );
    }

    fn graph_reorder(&mut self, _: &jack::Client) -> jack::Control {
        // println!("JACK: graph reordered");
        jack::Control::Continue
    }

    fn xrun(&mut self, _: &jack::Client) -> jack::Control {
        // println!("JACK: xrun occurred");
        jack::Control::Continue
    }
}
