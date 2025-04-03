use crate::core::collections::VecDeque;

use crate::core::{string::String, vec::Vec};

use knaster_core::Seconds;

use crate::graph::GraphError;

pub fn graph_log_error(e: GraphError) {
    match e {
        GraphError::SendToGraphGen(_) => todo!(),
        GraphError::NodeNotFound => todo!(),
        GraphError::InputOutOfBounds(_) => todo!(),
        GraphError::OutputOutOfBounds(_) => todo!(),
        GraphError::GraphInputOutOfBounds(_) => todo!(),
        GraphError::GraphOutputOutOfBounds(_) => todo!(),
        GraphError::ParameterDescriptionNotFound(_) => todo!(),
        GraphError::ParameterIndexOutOfBounds(_) => todo!(),
        GraphError::ParameterError(parameter_error) => todo!(),
        GraphError::PushChangeError(_) => todo!(),
        GraphError::WrongSourceNodeGraph {
            expected_graph,
            found_graph,
        } => todo!(),
        GraphError::WrongSinkNodeGraph {
            expected_graph,
            found_graph,
        } => todo!(),
    }
}
pub fn graph_log_warn(e: GraphError) {}

/// A log message sent from the audio thread, requiring no allocations.
///
/// A full message consists of any number of `ArLogMessage` values followed by `ArLogMessage::End`
#[derive(Clone, Copy, Debug)]
pub enum ArLogMessage {
    Str(&'static str),
    Float(f64),
    Integer(u64),
    Timestamp(Seconds),
    End,
}
impl From<&'static str> for ArLogMessage {
    fn from(value: &'static str) -> Self {
        ArLogMessage::Str(value)
    }
}
impl From<f64> for ArLogMessage {
    fn from(value: f64) -> Self {
        ArLogMessage::Float(value)
    }
}
impl From<f32> for ArLogMessage {
    fn from(value: f32) -> Self {
        ArLogMessage::Float(value as f64)
    }
}
impl From<u64> for ArLogMessage {
    fn from(value: u64) -> Self {
        ArLogMessage::Integer(value)
    }
}
impl From<u32> for ArLogMessage {
    fn from(value: u32) -> Self {
        ArLogMessage::Integer(value as u64)
    }
}
impl From<u16> for ArLogMessage {
    fn from(value: u16) -> Self {
        ArLogMessage::Integer(value as u64)
    }
}
impl From<u8> for ArLogMessage {
    fn from(value: u8) -> Self {
        ArLogMessage::Integer(value as u64)
    }
}
impl From<Seconds> for ArLogMessage {
    fn from(value: Seconds) -> Self {
        ArLogMessage::Timestamp(value)
    }
}

pub struct ArLogReceiver {
    receivers: Vec<rtrb::Consumer<ArLogMessage>>,
    received_messages: VecDeque<ArLogMessage>,
}
impl ArLogReceiver {
    pub fn new() -> Self {
        Self {
            receivers: Vec::new(),
            received_messages: VecDeque::with_capacity(10),
        }
    }
    /// Receive messages and store them internally. Only full message chains are received ending
    /// with `AtLogMessage::End`.
    pub fn recv(&mut self) {
        for rec in &mut self.receivers {
            let slots = rec.slots();
            if let Ok(read_chunk) = rec.read_chunk(slots) {
                let (s0, s1) = read_chunk.as_slices();
                let mut last_end = 0;
                while let Some(pos) = s0
                    .iter()
                    .chain(s1)
                    .skip(last_end)
                    .position(|m| matches!(m, &ArLogMessage::End))
                {
                    for m in s0.iter().chain(s1).skip(last_end).take(pos) {
                        self.received_messages.push_back(*m);
                    }
                    last_end += pos;
                }
                read_chunk.commit(last_end);
            }
        }
    }
    /// Log received messages using log::info
    pub fn log(&mut self) {}
    pub fn sender(&mut self) -> ArLogSender {
        let (tx, rx) = rtrb::RingBuffer::new(100);
        self.receivers.push(rx);
        ArLogSender { sender: tx }
    }
}
pub struct ArLogSender {
    sender: rtrb::Producer<ArLogMessage>,
}
impl ArLogSender {
    pub fn send(&mut self, message: impl Into<ArLogMessage>) {
        self.sender.push(message.into()).ok();
    }
}

macro_rules! rt_log {
    () => {

    };
    // Decompose multiple `eval`s recursively
    ($logger:ident; $($es:expr),+) => {{
        $logger
        rt_log! { eval $e }
        calculate! { $(eval $es),+ }
    }};
}
