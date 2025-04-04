use crate::core::{collections::VecDeque, vec::Vec};

use knaster_primitives::Seconds;

/// A log message sent from the audio thread, requiring no allocations.
///
/// A full message consists of any number of `ArLogMessage` values followed by `ArLogMessage::End`
#[derive(Clone, Copy, Debug)]
pub enum ArLogMessage {
    Str(&'static str),
    Float(f64),
    Unsigned(u64),
    Signed(i64),
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
impl From<usize> for ArLogMessage {
    fn from(value: usize) -> Self {
        ArLogMessage::Unsigned(value as u64)
    }
}
impl From<u64> for ArLogMessage {
    fn from(value: u64) -> Self {
        ArLogMessage::Unsigned(value)
    }
}
impl From<u32> for ArLogMessage {
    fn from(value: u32) -> Self {
        ArLogMessage::Unsigned(value as u64)
    }
}
impl From<u16> for ArLogMessage {
    fn from(value: u16) -> Self {
        ArLogMessage::Unsigned(value as u64)
    }
}
impl From<u8> for ArLogMessage {
    fn from(value: u8) -> Self {
        ArLogMessage::Unsigned(value as u64)
    }
}
impl From<i64> for ArLogMessage {
    fn from(value: i64) -> Self {
        ArLogMessage::Signed(value)
    }
}
impl From<i32> for ArLogMessage {
    fn from(value: i32) -> Self {
        ArLogMessage::Signed(value as i64)
    }
}
impl From<i16> for ArLogMessage {
    fn from(value: i16) -> Self {
        ArLogMessage::Signed(value as i64)
    }
}
impl From<i8> for ArLogMessage {
    fn from(value: i8) -> Self {
        ArLogMessage::Signed(value as i64)
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
    pub fn send(&mut self, message: ArLogMessage) {
        self.sender.push(message).ok();
    }
}

#[macro_export]
macro_rules! rt_log {
    ($logger:expr; $($msg:expr),* $(,)?) => {{
    {
    use $crate::log::ArLogMessage;
        $(
            $logger.send(ArLogMessage::from($msg));
        )*
        $logger.send(ArLogMessage::End);
    }
    }};
}
#[cfg(test)]
mod tests {
    use super::ArLogReceiver;

    #[test]
    fn log_rt() {
        let mut log_receiver = ArLogReceiver::new();
        let mut logger = log_receiver.sender();
        rt_log!(logger; "En", 10, " mängd olika ", 5.0, 4.0_f32, 3.0_f64);
    }
}
