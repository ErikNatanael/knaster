//! Handles allow you to interact ergonomically with a node after it has been added to a graph.
//!
//! Roadmap:
//! - Typesafe Handle types

use crate::{
    SchedulingEvent, SchedulingToken, SharedFrameClock, Time,
    core::marker::PhantomData,
    graph::NodeOrGraph,
    graph::{GraphError, NodeId},
};
use knaster_core::{
    Param, ParameterError, ParameterHint, ParameterSmoothing, ParameterValue, Seconds, UGen,
    typenum::Unsigned,
};
/// no_std_compat prelude import, supporting both std and no_std
use std::prelude::v1::*;

use crate::core::sync::{Arc, Mutex};

use crate::SchedulingChannelProducer;

#[derive(Clone, Copy, Debug)]
/// A subset of a node's channels. Can be input or output channels depending on the context.
pub struct NodeSubset {
    #[allow(unused)]
    pub(crate) node: NodeOrGraph,
    /// The number of channels to produce. `start_channel + ` is the
    /// last channel in the subset.
    #[allow(unused)]
    pub(crate) channels: u16,
    /// The offset from the start of the channels of the node
    #[allow(unused)]
    pub(crate) start_channel: u16,
}

/// This is used to send scheduling events to the audio thread.
#[derive(Clone, Debug)]
pub struct SchedulingChannelSender(pub(crate) Arc<Mutex<SchedulingChannelProducer>>);
impl SchedulingChannelSender {
    /// Send a scheduling event to the audio thread.
    ///
    /// # Errors
    ///
    /// Returns an error if the graph this sender is connected to was freed or the event channel is
    /// full.
    pub fn send(&self, event: SchedulingEvent) -> Result<(), GraphError> {
        // no_std_compat uses `spin` replacements for Mutex, which has a different API.
        #[cfg(feature = "std")]
        {
            // Lock should never be poisoned, but if it is we don't care.
            let mut sender = match self.0.lock() {
                Ok(s) => s,
                Err(s) => s.into_inner(),
            };
            if sender.is_abandoned() {
                // A fence might be required, see: https://docs.rs/rtrb/latest/rtrb/struct.Producer.html#method.is_abandoned
                // std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
                return Err(ParameterError::GraphWasFreed.into());
            }
            sender
                .push(event)
                .map_err(|e| GraphError::PushChangeError(e.to_string()))?;
        }
        #[cfg(not(feature = "std"))]
        {
            let mut sender = self.0.lock();
            sender
                .push(event)
                .map_err(|e| GraphError::PushChangeError(e.to_string()))?;
        }
        Ok(())
    }

    /// Returns true if the graph this sender is connected to is still alive.
    pub fn is_alive(&self) -> bool {
        #[cfg(feature = "std")]
        match self.0.lock() {
            Ok(s) => !s.is_abandoned(),
            _ => false,
        }
        // `spin` mutexes don't have a `is_abandoned` method, so we assume true
        #[cfg(not(feature = "std"))]
        true
    }
}

/// Same as [`Handle<T>`], but the type is removed and instead, all the relevant information is
/// stored in the struct. This is somewhat less efficient, but often required or significantly more
/// ergonomic.
#[derive(Clone)]
pub struct AnyHandle {
    pub(crate) raw_handle: RawHandle,
    pub(crate) parameters: Vec<&'static str>,
    pub(crate) parameter_hints: Vec<ParameterHint>,
    pub(crate) inputs: u16,
    pub(crate) outputs: u16,
}

/// Handle to a node with its type erased. This allows a less safe interaction with the node, but the handle can easily be stored.
#[derive(Clone)]
pub(crate) struct RawHandle {
    pub(crate) node: NodeId,
    /// Allows us to send parameter changes straight to the audio thread
    sender: SchedulingChannelSender,
    shared_frame_clock: SharedFrameClock,
}
impl RawHandle {
    pub fn new(
        node: NodeId,
        sender: SchedulingChannelSender,
        shared_frame_clock: SharedFrameClock,
    ) -> Self {
        Self {
            node,
            sender,
            shared_frame_clock,
        }
    }
    pub fn is_alive(&self) -> bool {
        self.sender.is_alive()
    }
    pub fn send(&self, event: SchedulingEvent) -> Result<(), GraphError> {
        self.sender.send(event)
    }
    pub fn node_id(&self) -> NodeId {
        self.node
    }
    pub fn current_frame_time(&self) -> Seconds {
        self.shared_frame_clock.get()
    }
}

/// Handle with type data intact, without owning a T. Enables interacting with a
/// live node in a Graph, e.g. freeing and parameter changes. Allows local error
/// checking.
pub struct Handle<T> {
    _phantom: PhantomData<fn(&mut T) -> ()>,
    pub(crate) raw_handle: RawHandle,
}
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self {
            _phantom: self._phantom,
            raw_handle: self.raw_handle.clone(),
        }
    }
}

impl<T: UGen> Handle<T> {
    pub(crate) fn new(untyped_handle: RawHandle) -> Self {
        Self {
            _phantom: PhantomData,
            raw_handle: untyped_handle,
        }
    }
    /// Convert this handle into an [`AnyHandle`].
    pub fn into_any(self) -> AnyHandle {
        AnyHandle {
            raw_handle: self.raw_handle,
            parameters: T::param_descriptions().into_iter().collect(),
            parameter_hints: T::param_hints().into_iter().collect(),
            inputs: T::Inputs::U16,
            outputs: T::Outputs::U16,
        }
    }
    /// Get the number of inputs of this node.
    pub fn inputs(&self) -> usize {
        T::Inputs::USIZE
    }
    /// Get the number of outputs of this node.
    pub fn outputs(&self) -> usize {
        T::Outputs::USIZE
    }
}
// impl<T> From<&Handle<T>> for NodeId {
//     fn from(value: &Handle<T>) -> Self {
//         value.raw_handle.node
//     }
// }
impl<T: HandleTrait> From<&T> for NodeId {
    fn from(value: &T) -> Self {
        value.node_id()
    }
}
/// Trait for handles to nodes.
pub trait HandleTrait: Sized {
    /// Set a parameter value on this node.
    fn set<C: Into<ParameterChange>>(&self, change: C) -> Result<(), GraphError>;
    /// Send a [`SchedulingEvent`] to the audio thread.
    fn schedule_event(&self, event: SchedulingEvent) -> Result<(), GraphError>;
    /// Get the [`NodeId`] of this node.
    fn node_id(&self) -> NodeId;
    /// Get the number of inputs of this node.
    fn inputs(&self) -> u16;
    /// Get the number of outputs of this node.
    fn outputs(&self) -> u16;
    /// Get a subset of this node's channels.
    fn subset(&self, start_channel: u16, channels: u16) -> NodeSubset {
        NodeSubset {
            node: NodeOrGraph::Node(self.node_id()),
            channels,
            start_channel,
        }
    }
    /// Get parameter descriptions for this node.
    fn parameters(&self) -> Vec<&'static str>;
    /// Get parameter hints for this node.
    fn hints(&self) -> Vec<ParameterHint>;

    /// Returns time of the Runner connected to this
    fn current_frame_time(&self) -> Seconds;
    /// True if it is still possible to send values. This does not necessarily mean that the node
    /// exists.
    ///
    /// Parameter changes sent to non-existing nodes will eventually be cleaned up, but they may
    /// fill the graph buffer before that.
    fn can_send(&self) -> bool;
}
impl<T: UGen> HandleTrait for Handle<T> {
    fn set<C: Into<ParameterChange>>(&self, change: C) -> Result<(), GraphError> {
        let c = change.into();
        let param_index = match c.param {
            knaster_core::Param::Index(param_i) => param_i,
            knaster_core::Param::Desc(desc) => {
                match T::param_descriptions().iter().position(|d| *d == desc) {
                    Some(param_i) => param_i,
                    _ => {
                        // Fail
                        return Err(ParameterError::DescriptionNotFound(desc).into());
                    }
                }
            }
        };
        let event = SchedulingEvent {
            node_key: self.raw_handle.node.key(),
            parameter: param_index,
            value: c.value,
            smoothing: c.smoothing,
            token: c.token,
            time: c.time,
        };
        self.raw_handle.send(event)
    }

    fn schedule_event(&self, event: SchedulingEvent) -> Result<(), GraphError> {
        self.raw_handle.send(event)
    }

    fn node_id(&self) -> NodeId {
        self.raw_handle.node_id()
    }

    fn inputs(&self) -> u16 {
        T::Inputs::U16
    }

    fn outputs(&self) -> u16 {
        T::Outputs::U16
    }

    fn current_frame_time(&self) -> Seconds {
        self.raw_handle.current_frame_time()
    }

    fn can_send(&self) -> bool {
        self.raw_handle.is_alive()
    }

    fn parameters(&self) -> Vec<&'static str> {
        T::param_descriptions().to_vec()
    }

    fn hints(&self) -> Vec<ParameterHint> {
        T::param_hints().to_vec()
    }
}
impl HandleTrait for AnyHandle {
    fn set<C: Into<ParameterChange>>(&self, change: C) -> Result<(), GraphError> {
        let c = change.into();
        let param_index = match c.param {
            knaster_core::Param::Index(param_i) => param_i,
            knaster_core::Param::Desc(desc) => {
                if let Some(param_i) = self.parameters.iter().position(|d| *d == desc) {
                    param_i
                } else {
                    // Fail
                    return Err(ParameterError::DescriptionNotFound(desc).into());
                }
            }
        };
        let event = SchedulingEvent {
            node_key: self.raw_handle.node.key(),
            parameter: param_index,
            value: c.value,
            smoothing: c.smoothing,
            token: c.token,
            time: c.time,
        };
        self.raw_handle.send(event)
    }

    fn schedule_event(&self, event: SchedulingEvent) -> Result<(), GraphError> {
        self.raw_handle.send(event)
    }

    fn node_id(&self) -> NodeId {
        self.raw_handle.node_id()
    }

    fn inputs(&self) -> u16 {
        self.inputs
    }

    fn outputs(&self) -> u16 {
        self.outputs
    }

    fn current_frame_time(&self) -> Seconds {
        self.raw_handle.current_frame_time()
    }

    fn can_send(&self) -> bool {
        self.raw_handle.is_alive()
    }

    fn parameters(&self) -> Vec<&'static str> {
        self.parameters.clone()
    }

    fn hints(&self) -> Vec<ParameterHint> {
        self.parameter_hints.clone()
    }
}
/// A parameter change API for the [`HandleTrait`] (deprecated).
#[derive(Debug)]
pub struct ParameterChange2<'a, H: HandleTrait> {
    handle: &'a H,
    param: usize,
    value: Option<ParameterValue>,
    smoothing: Option<ParameterSmoothing>,
    token: Option<SchedulingToken>,
    time: Option<Time>,
    was_sent: bool,
}
impl<H: HandleTrait> ParameterChange2<'_, H> {
    /// Send a trigger parameter change.
    pub fn trig(mut self) -> Self {
        self.value = Some(ParameterValue::Trigger);
        self
    }
    /// Send a parameter change with the given value.
    pub fn value(mut self, v: impl Into<ParameterValue>) -> Self {
        self.value = Some(v.into());
        self
    }
    /// Set the smoothing setting for the parameter.
    pub fn smooth(mut self, v: impl Into<ParameterSmoothing>) -> Self {
        self.smoothing = Some(v.into());
        self
    }
    /// Use a scheduling token to activate the parameter change.
    pub fn token(mut self, v: impl Into<SchedulingToken>) -> Self {
        self.token = Some(v.into());
        self
    }
    /// Apply the parameter change after the given time.
    pub fn after(mut self, v: impl Into<Seconds>) -> Self {
        self.time = Some(Time::after(v.into()));
        self
    }
    /// Apply the parameter change at the given time.
    pub fn at(mut self, v: impl Into<Time>) -> Self {
        let t = v.into().to_absolute();
        self.time = Some(t);
        self
    }
    /// Send the parameter change.
    pub fn send(mut self) -> Result<(), GraphError> {
        self.was_sent = true;

        self.handle.schedule_event(SchedulingEvent {
            node_key: self.handle.node_id().key(),
            parameter: self.param,
            value: self.value,
            smoothing: self.smoothing,
            token: self.token.take(),
            time: self.time.take(),
        })
    }
}
impl<H: HandleTrait> Drop for ParameterChange2<'_, H> {
    fn drop(&mut self) {
        if !self.was_sent {
            if let Err(e) = self.handle.schedule_event(SchedulingEvent {
                node_key: self.handle.node_id().key(),
                parameter: self.param,
                value: self.value,
                smoothing: self.smoothing,
                token: self.token.take(),
                time: self.time.take(),
            }) {
                log::error!("Error sending parameter change: {e}");
            }
        }
    }
}
/// Parameter change API (deprecated).
#[derive(Clone, Debug)]
pub struct ParameterChange {
    param: Param,
    value: Option<ParameterValue>,
    smoothing: Option<ParameterSmoothing>,
    token: Option<SchedulingToken>,
    time: Option<Time>,
}
impl<P: Into<Param>, V: Into<ParameterValue>> From<(P, V)> for ParameterChange {
    fn from((param, value): (P, V)) -> Self {
        ParameterChange {
            param: param.into(),
            value: Some(value.into()),
            smoothing: None,
            token: None,
            time: None,
        }
    }
}
impl<P: Into<Param>, V: Into<ParameterValue>> From<(P, V, SchedulingToken)> for ParameterChange {
    fn from((param, value, token): (P, V, SchedulingToken)) -> Self {
        ParameterChange {
            param: param.into(),
            value: Some(value.into()),
            smoothing: None,
            token: Some(token),
            time: None,
        }
    }
}
impl<P: Into<Param>, V: Into<ParameterValue>, S: Into<ParameterSmoothing>>
    From<(P, V, S, SchedulingToken)> for ParameterChange
{
    fn from((param, value, smoothing, token): (P, V, S, SchedulingToken)) -> Self {
        ParameterChange {
            param: param.into(),
            value: Some(value.into()),
            smoothing: Some(smoothing.into()),
            token: Some(token),
            time: None,
        }
    }
}
impl<P: Into<Param>, V: Into<ParameterValue>, S: Into<ParameterSmoothing>> From<(P, V, S)>
    for ParameterChange
{
    fn from((param, value, smoothing): (P, V, S)) -> Self {
        ParameterChange {
            param: param.into(),
            value: Some(value.into()),
            smoothing: Some(smoothing.into()),
            token: None,
            time: None,
        }
    }
}

// impl<Sample, T: Gen<Sample = Sample>> Handleable for T {
//     type HandleType = Handle<Self>;

//     fn get_handle(untyped_handle: UntypedHandle) -> Self::HandleType {
//         Handle::<Self>::new(untyped_handle)
//     }
// }

// We might do per Gen handle types in the future. Leaving this here for then:
// pub struct OscHandle<F>(Handle<Self>);
// impl<F: Float> OscHandle<F> {
//     pub fn freq(&mut self) {
//         todo!()
//     }
// }
// impl<F: Float> HandleTrait for OscHandle<F> {
//     fn set(&mut self) {
//         todo!()
//     }
//     fn from_untyped(untyped_handle: UntypedHandle) -> Self {
//         OscHandle::<F>(Handle::new(untyped_handle))
//     }
// }
// impl<F: Float> Handleable for Osc<F> {
//     type HandleType = OscHandle<F>;
// }
// impl<F: Float> Handleable for crate::test_reverb::Reverb<F> {
//     type HandleType = Handle<Self>;
// }
