//! Infrastructure for scheduling changes in a [`Graph`]
//!
//! There are different motivations for scheduling:
//! 1. specific timing of events
//! 2. synchronised application of changes
//!
//! Likewise, there are different kind of changes to a graph, which need different solutions:
//! a. Add a new node
//! b. Remove an existing node
//! c. Change a parameter value
//!
//! 1. is accomplished by multiple systems
//!
//! 2. is mostly accomplished by using [`SchedulingToken`]

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;

#[cfg(feature = "std")]
use std::sync::Arc;

use crate::{
    core::sync::atomic::{AtomicBool, AtomicU64},
    graph::NodeKey,
};

use knaster_core::{ParameterError, ParameterSmoothing, ParameterValue, Seconds};

#[derive(Debug, Clone)]
pub struct SchedulingEvent {
    pub(crate) node_key: NodeKey,
    pub(crate) parameter: usize,
    pub(crate) value: Option<ParameterValue>,
    pub(crate) smoothing: Option<ParameterSmoothing>,
    pub(crate) token: Option<SchedulingToken>,
    pub(crate) time: Option<SchedulingTime>,
}
pub(crate) type SchedulingChannelProducer = rtrb::Producer<SchedulingEvent>;
// Every GraphGen has one of these for receiving parameter changes.
pub(crate) type SchedulingChannelConsumer = rtrb::Consumer<SchedulingEvent>;

pub enum SchedulingError {
    ParameterError(ParameterError),
}

#[derive(Clone, Debug)]
pub struct SharedFrameClock(Arc<AtomicU64>);
impl SharedFrameClock {
    pub(crate) fn new() -> Self {
        Self(Arc::new(AtomicU64::new(0)))
    }
    /// Only the Runner should set the time using this function
    pub(crate) fn store_new_time(&mut self, new_time: Seconds) {
        let as_u64 = unsafe { crate::core::mem::transmute::<Seconds, u64>(new_time) };
        self.0.store(as_u64, core::sync::atomic::Ordering::Relaxed);
    }
    pub fn get(&self) -> Seconds {
        let as_u64 = self.0.load(core::sync::atomic::Ordering::Relaxed);
        unsafe { crate::core::mem::transmute::<u64, Seconds>(as_u64) }
    }
}

/// The time something should be scheduled to start.
///
/// The time can be relative to when the event is received on the audio thread, or in absolute
/// samples. When converting from primitives
#[derive(Clone, Debug)]
pub struct SchedulingTime {
    seconds: Seconds,
    absolute: bool,
}
impl SchedulingTime {
    /// Returns the number of samples until this event should be applied. If the timing is
    /// relative, the counter is also decremented.
    pub fn to_samples_until_due(
        &mut self,
        block_size: u64,
        sample_rate: u64,
        frame_clock: u64,
    ) -> u64 {
        if self.absolute {
            let t = self.seconds.to_samples(sample_rate);
            // TODO: Real logging
            #[cfg(debug_assertions)]
            if t < frame_clock {
                std::eprintln!("Event was scheduled late {}, {}", t, frame_clock);
            }
            t.saturating_sub(frame_clock)
        } else {
            let samples = self.seconds.to_samples(sample_rate);
            self.seconds = self
                .seconds
                .saturating_sub(Seconds::from_samples(block_size, sample_rate));
            samples
        }
    }
    pub fn absolute(seconds: Seconds) -> Self {
        Self {
            seconds,
            absolute: true,
        }
    }
    pub fn relative(seconds: Seconds) -> Self {
        Self {
            seconds,
            absolute: false,
        }
    }
    pub fn to_absolute(mut self) -> Self {
        self.absolute = true;
        self
    }
    pub fn to_relative(mut self) -> Self {
        self.absolute = false;
        self
    }
}
impl From<Seconds> for SchedulingTime {
    fn from(value: Seconds) -> Self {
        SchedulingTime {
            seconds: value,
            absolute: false,
        }
    }
}

/// Attach this token to all changes that you want to be simultaneous. Then,
/// send the token to the outermost graph that is affected. Use the top level
/// graph if in doubt. None of the changes will be applied until the token is
/// activated.
#[derive(Clone, Debug)]
pub struct SchedulingToken {
    token: Arc<AtomicBool>,
}
impl Default for SchedulingToken {
    fn default() -> Self {
        Self::new()
    }
}

impl SchedulingToken {
    pub fn new() -> Self {
        Self {
            token: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn ready(&self) -> bool {
        self.token.load(crate::core::sync::atomic::Ordering::SeqCst)
    }
    /// Activate the token
    /// The token will send itself to the top level graph where it will be
    /// activated at the start of a block. This is done to ensure that all
    /// dependents of the token are activated in the same block (i.e. at the
    /// same audio time) without data races.
    ///
    /// NB: Don't call this from the audio thread! This function is not
    /// guaranteed to be wait free.
    pub fn activate(self) {
        // Send self to the top level graph to be activated at the start of a block
        todo!()
    }
    /// Activates the token immediately
    ///
    /// NB: This function should only be called from the audio thread, otherwise
    /// prefer [`SchedulingToken::activate`]. If activated outside of the audio
    /// thread changes aren't guaranteed to be applied in the same block.
    pub fn activate_inner(self) {
        self.token.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

// pub trait Schedulable {
//     fn set(&mut self, event: impl Into<SchedulingEvent>) -> Result<(), ()>;
// }

// impl<Sample: Float, T: Gen<Sample = Sample> + Parameterable<Sample>> Schedulable for Handle<T> {
//     fn set(&mut self, event: impl Into<SchedulingEvent>) -> Result<(), ()> {
//         todo!()
//     }
// }
