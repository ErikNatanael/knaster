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

use crate::{core::sync::atomic::AtomicBool, graph::NodeKey};

use knaster_core::{Float, Param, ParameterError, ParameterSmoothing, ParameterValue};

pub struct SchedulingEvent {
    pub(crate) node_key: NodeKey,
    pub(crate) parameter: Param,
    pub(crate) value: Option<ParameterValue>,
    pub(crate) smoothing: Option<ParameterSmoothing>,
    pub(crate) token: Option<SchedulingToken>,
}
pub(crate) type SchedulingChannelProducer = rtrb::Producer<SchedulingEvent>;
// Every GraphGen has one of these for receiving parameter changes.
pub(crate) type SchedulingChannelConsumer = rtrb::Consumer<SchedulingEvent>;

/// Receives and applies scheduled events on the audio thread.
///
/// One of these is run at the start of every block for every graph.
pub struct AudioThreadScheduler {
    incoming_events: SchedulingChannelConsumer,
}

pub enum SchedulingError {
    ParameterError(ParameterError),
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
