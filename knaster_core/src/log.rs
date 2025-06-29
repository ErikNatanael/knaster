//! Logging utilities for logging from the audio thread.
//!
//! This is a simple logging system for sending messages from the audio thread without allocations.
//!
//! Messages are sent as a chain of `ArLogMessage` values. The chain is terminated with
//! `ArLogMessage::End`. Messages are sent in a preallocated channel.
use core::{fmt::Display, mem::MaybeUninit};

use knaster_primitives::{
    Seconds, Size,
    numeric_array::{self, NumericArray},
    typenum::{Add1, B1, U0},
};

/// A log message sent from the audio thread, requiring no allocations.
///
/// A full message consists of any number of `ArLogMessage` values followed by `ArLogMessage::End`
#[derive(Clone, Copy, Debug)]
pub enum ArLogMessage {
    /// A string message.
    Str(&'static str),
    /// A float message.
    Float(f64),
    /// An unsigned integer message.
    Unsigned(u64),
    /// A signed integer message.
    Signed(i64),
    /// A timestamp message in [`Seconds`].
    Timestamp(Seconds),
    /// Marks the end of a message chain.
    End,
}
impl ArLogMessage {
    /// Returns true if this is the end messages of a message chain.
    pub fn is_end(&self) -> bool {
        matches!(self, ArLogMessage::End)
    }
}
impl Display for ArLogMessage {
    fn fmt(&self, f: &mut crate::core::fmt::Formatter<'_>) -> crate::core::fmt::Result {
        match self {
            ArLogMessage::Str(s) => write!(f, "{}", s),
            ArLogMessage::Float(n) => write!(f, "{}", n),
            ArLogMessage::Unsigned(u) => write!(f, "{}", u),
            ArLogMessage::Signed(i) => write!(f, "{}", i),
            ArLogMessage::Timestamp(s) => write!(f, "{} seconds", s.to_secs_f64()),
            ArLogMessage::End => write!(f, "End"),
        }
    }
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

// TODO: Make the array of receivers static with a generic sender() method that returns a new
// ArLogReceiver
/// A receiver for log messages.
///
/// The Size parameter determines the number of preallocated channels, and therefore the number of
/// supported threads to receive log messages from. Messages are received through ring buffers, one
/// per thread.
pub struct ArLogReceiver<N: Size> {
    receivers: NumericArray<rtrb::Consumer<ArLogMessage>, N>,
}
impl ArLogReceiver<U0> {
    /// Create a new ArLogReceiver with no channels.
    ///
    /// Use [`Self::sender`] to add channels to receive from.
    ///
    /// # Example
    /// ```rust
    /// use knaster_core::log::ArLogReceiver;
    /// let receiver = ArLogReceiver::new(); // 0 channels
    /// let (sender0, receiver) = receiver.sender(100); // 1 channel
    /// let (sender1, receiver) = receiver.sender(100); // 2 channels
    /// // `receiver` now contains two channels
    /// assert_eq!(receiver.channels(), 2);
    /// ````
    pub fn new() -> Self {
        Self {
            receivers: NumericArray::from([]),
        }
    }
}

impl Default for ArLogReceiver<U0> {
    fn default() -> Self {
        Self::new()
    }
}
impl<N: Size> ArLogReceiver<N> {
    /// Receive messages. A full or partial message chain is passed to `log_handler`.
    ///
    /// Only full message chains are received, i.e. those ending
    /// with `AtLogMessage::End`, but they may be split into two calls to the `log_handler`.
    ///
    /// Each call to the log_handler may or may not contain a full message chain. If a message chain is not complete,
    /// the remaining messages are passed to the next call to log_handler.
    pub fn recv(&mut self, mut log_handler: impl FnMut(&[ArLogMessage])) {
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
                    let slice0 = if last_end >= s0.len() {
                        &[]
                    } else {
                        &s0[last_end..(last_end + pos + 1).min(s0.len())]
                    };
                    let slice1 = if last_end + pos < s0.len() {
                        &[]
                    } else if last_end >= s0.len() {
                        &s1[(last_end - s0.len())..=(last_end + pos - s0.len())]
                    } else {
                        &s1[0..=(last_end + pos - s0.len())]
                    };
                    log_handler(slice0);
                    log_handler(slice1);
                    // for m in s0.iter().chain(s1).skip(last_end).take(pos) {
                    //     self.received_messages.push_back(*m);
                    // }
                    last_end += pos + 1;
                }
                read_chunk.commit(last_end);
            }
        }
    }

    /// Add a new sender to this receiver. Consumes `self` and produces a new [`ArLogReceiver`]
    /// with one more channel.
    ///
    /// # Example
    /// ```rust
    /// use knaster_core::log::ArLogReceiver;
    /// let receiver = ArLogReceiver::new(); // 0 channels
    /// // Add a channel with a capacity of 100 messages at a time:
    /// let (sender0, receiver) = receiver.sender(100);     
    /// // Add another channel with a capacity of 20messages at a time:
    /// let (sender1, receiver) = receiver.sender(20);
    /// // `receiver` now contains two channels
    /// assert_eq!(receiver.channels(), 2);
    /// ````
    pub fn sender(self, capacity: usize) -> (ArLogSender, ArLogReceiver<Add1<N>>)
    where
        N: core::ops::Add<B1>,
        <N as core::ops::Add<B1>>::Output: Size,
    {
        let (tx, rx) = rtrb::RingBuffer::new(capacity);
        let mut array: numeric_array::generic_array::GenericArray<MaybeUninit<_>, Add1<N>> =
            numeric_array::generic_array::GenericArray::uninit();

        // Copy existing elements
        for (i, p) in self.receivers.into_iter().enumerate() {
            array[i].write(p);
        }

        // Write new element
        array[N::USIZE].write(rx);

        // SAFETY: All items are initialized
        let receivers = unsafe {
            NumericArray::from(numeric_array::generic_array::GenericArray::assume_init(
                array,
            ))
        };
        (ArLogSender::RingBuffer(tx), ArLogReceiver { receivers })
    }
    /// Returns the number of receiver channels this [`ArLogReceiver`] receivs from.
    pub fn channels(&self) -> usize {
        N::USIZE
    }
}
/// Sender of [`ArLogMessage`]s. Used for logging from the audio thread without allocations.
///
/// Usually acquired through `AudioCtx` in `knaster_graph`. Use [`Self::non_rt`] to create an
/// [`ArLogSender`] for a non real-time context, e.g. in testing.
pub enum ArLogSender {
    /// Logs via a ring buffer to an [`ArLogReceiver`].
    RingBuffer(rtrb::Producer<ArLogMessage>),
    /// Logs to the `log` scaffolding from the `log` crate.
    Log,
}
impl ArLogSender {
    /// Create a fallback `ArLogSender` which logs via the `log` crate instead of to an
    /// `ArLogReceiver`. See the `log` crate for more info on how to receive the log messages.
    pub fn non_rt() -> Self {
        ArLogSender::Log
    }
    /// Send a single log message. It is recommended to use the `rt_log` macro instead, since it
    /// is more convenient and automatically adds the `End` message to the end of a chain.
    pub fn send(&mut self, message: ArLogMessage) {
        match self {
            ArLogSender::RingBuffer(sender) => {
                sender.push(message).ok();
            }
            ArLogSender::Log => log::warn!("{}", message),
        }
    }
}

/// Macro for sending [`ArLogMessage`]s via an [`ArLogSender`] to an [`ArLogReceiver`].
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
        let log_receiver = ArLogReceiver::new();
        let (mut logger, _log_receiver) = log_receiver.sender(20);
        rt_log!(logger; "En", 10, " mängd olika ", 5.0, 4.0_f32, 3.0_f64);
    }
}
