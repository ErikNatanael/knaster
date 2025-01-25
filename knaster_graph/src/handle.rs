//! Handles allow you to interact ergonomically with a node after it has been added to a graph.
//!
//! Roadmap:
//! - Typesafe Handle types

use crate::{
    core::{eprintln, marker::PhantomData},
    graph::{GraphError, NodeId},
    SchedulingEvent, SchedulingTime, SchedulingToken, SharedFrameClock,
};
use alloc::{string::ToString, vec::Vec};
use knaster_core::{
    typenum::Unsigned, Param, ParameterError, ParameterSmoothing, ParameterValue, Seconds, UGen,
};

#[cfg(not(feature = "std"))]
use alloc::sync::{Arc, Mutex};
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};

use crate::SchedulingChannelProducer;

/// Same as [`Handle<T>`], but the type is removed and instead, all the relevant information is
/// stored in the struct. This is somewhat less efficient, but often required or significantly more
/// ergonomic.
pub struct AnyHandle {
    raw_handle: RawHandle,
    parameters: Vec<&'static str>,
    inputs: usize,
    outputs: usize,
}

/// Handle to a node with its type erased. This allows a less safe interaction with the node, but the handle can easily be stored.
#[derive(Clone)]
pub(crate) struct RawHandle {
    pub(crate) node: NodeId,
    /// Allows us to send parameter changes straight to the audio thread
    sender: Arc<Mutex<SchedulingChannelProducer>>,
    shared_frame_clock: SharedFrameClock,
}
impl RawHandle {
    pub fn new(
        node: NodeId,
        sender: Arc<Mutex<SchedulingChannelProducer>>,
        shared_frame_clock: SharedFrameClock,
    ) -> Self {
        Self {
            node,
            sender,
            shared_frame_clock,
        }
    }
    pub fn is_alive(&self) -> bool {
        if let Ok(s) = self.sender.lock() {
            !s.is_abandoned()
        } else {
            false
        }
    }
    pub fn send(&self, event: SchedulingEvent) -> Result<(), GraphError> {
        // Lock should never be poisoned, but if it is we don't care.
        let mut sender = match self.sender.lock() {
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
        Ok(())
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
    pub fn into_any(self) -> AnyHandle {
        AnyHandle {
            raw_handle: self.raw_handle,
            parameters: T::param_descriptions().into_iter().collect(),
            inputs: T::Inputs::USIZE,
            outputs: T::Outputs::USIZE,
        }
    }
    pub fn inputs(&self) -> usize {
        T::Inputs::USIZE
    }
    pub fn outputs(&self) -> usize {
        T::Outputs::USIZE
    }
}
impl<T> From<&Handle<T>> for NodeId {
    fn from(value: &Handle<T>) -> Self {
        value.raw_handle.node
    }
}
pub trait HandleTrait: Sized {
    fn set<C: Into<ParameterChange>>(&self, change: C) -> Result<(), GraphError>;
    fn change(&self, param: impl Into<Param>) -> Result<ParameterChange2<Self>, ParameterError>;
    fn schedule_event(&self, event: SchedulingEvent) -> Result<(), GraphError>;
    fn node_id(&self) -> NodeId;
    fn inputs(&self) -> usize;
    fn outputs(&self) -> usize;
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
                if let Some(param_i) = T::param_descriptions().iter().position(|d| *d == desc) {
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

    fn node_id(&self) -> NodeId {
        self.raw_handle.node_id()
    }

    fn inputs(&self) -> usize {
        T::Inputs::USIZE
    }

    fn outputs(&self) -> usize {
        T::Outputs::USIZE
    }

    fn schedule_event(&self, event: SchedulingEvent) -> Result<(), GraphError> {
        self.raw_handle.send(event)
    }

    fn change(&self, param: impl Into<Param>) -> Result<ParameterChange2<Self>, ParameterError> {
        let param_index = match param.into() {
            knaster_core::Param::Index(param_i) => {
                if param_i >= T::Parameters::USIZE {
                    return Err(ParameterError::ParameterIndexOutOfBounds);
                } else {
                    param_i
                }
            }
            knaster_core::Param::Desc(desc) => {
                if let Some(param_i) = T::param_descriptions().iter().position(|d| *d == desc) {
                    param_i
                } else {
                    // Fail
                    return Err(ParameterError::DescriptionNotFound(desc));
                }
            }
        };
        Ok(ParameterChange2 {
            handle: self,
            param: param_index,
            value: None,
            smoothing: None,
            token: None,
            time: None,
            was_sent: false,
        })
    }

    fn can_send(&self) -> bool {
        self.raw_handle.is_alive()
    }

    fn current_frame_time(&self) -> Seconds {
        self.raw_handle.current_frame_time()
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
    fn node_id(&self) -> NodeId {
        self.raw_handle.node_id()
    }

    fn inputs(&self) -> usize {
        self.inputs
    }

    fn outputs(&self) -> usize {
        self.outputs
    }

    fn schedule_event(&self, event: SchedulingEvent) -> Result<(), GraphError> {
        self.raw_handle.send(event)
    }

    fn change(&self, param: impl Into<Param>) -> Result<ParameterChange2<Self>, ParameterError> {
        let param_index = match param.into() {
            knaster_core::Param::Index(param_i) => {
                if param_i >= self.parameters.len() {
                    return Err(ParameterError::ParameterIndexOutOfBounds);
                } else {
                    param_i
                }
            }
            knaster_core::Param::Desc(desc) => {
                if let Some(param_i) = self.parameters.iter().position(|d| *d == desc) {
                    param_i
                } else {
                    return Err(ParameterError::DescriptionNotFound(desc));
                }
            }
        };
        Ok(ParameterChange2 {
            handle: self,
            param: param_index,
            value: None,
            smoothing: None,
            token: None,
            time: None,
            was_sent: false,
        })
    }

    fn can_send(&self) -> bool {
        self.raw_handle.is_alive()
    }

    fn current_frame_time(&self) -> Seconds {
        self.raw_handle.current_frame_time()
    }
}
// pub trait Handleable: Sized {
//     type HandleType: HandleTrait;
//     fn get_handle(untyped_handle: RawHandle) -> Self::HandleType {
//         Self::HandleType::from_untyped(untyped_handle)
//     }
// }
//
#[derive(Debug)]
pub struct ParameterChange2<'a, H: HandleTrait> {
    handle: &'a H,
    param: usize,
    value: Option<ParameterValue>,
    smoothing: Option<ParameterSmoothing>,
    token: Option<SchedulingToken>,
    time: Option<SchedulingTime>,
    was_sent: bool,
}
impl<H: HandleTrait> ParameterChange2<'_, H> {
    pub fn trig(mut self) -> Self {
        self.value = Some(ParameterValue::Trigger);
        self
    }
    pub fn value(mut self, v: impl Into<ParameterValue>) -> Self {
        self.value = Some(v.into());
        self
    }
    pub fn smooth(mut self, v: impl Into<ParameterSmoothing>) -> Self {
        self.smoothing = Some(v.into());
        self
    }
    pub fn token(mut self, v: impl Into<SchedulingToken>) -> Self {
        self.token = Some(v.into());
        self
    }
    pub fn after(mut self, v: impl Into<SchedulingTime>) -> Self {
        self.time = Some(v.into());
        self
    }
    pub fn at(mut self, v: impl Into<SchedulingTime>) -> Self {
        let t = v.into().to_absolute();
        self.time = Some(t);
        self
    }
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
                eprintln!("Error sending parameter change: {e}");
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct ParameterChange {
    param: Param,
    value: Option<ParameterValue>,
    smoothing: Option<ParameterSmoothing>,
    token: Option<SchedulingToken>,
    time: Option<SchedulingTime>,
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
