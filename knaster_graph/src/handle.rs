//! Handles allow you to interact ergonomically with a node after it has been added to a graph.
//!
//! Roadmap:
//! - Typesafe Handle types

use crate::{
    core::marker::PhantomData,
    graph::NodeId,
    SchedulingEvent, SchedulingTime, SchedulingToken,
};
use knaster_core::{
    numeric_array, typenum::Unsigned, AudioCtx, Gen, Param, ParameterError, ParameterRange,
    ParameterSmoothing, ParameterValue,
};

#[cfg(not(feature = "std"))]
use alloc::sync::{Arc, Mutex};
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};

use crate::SchedulingChannelProducer;

/// Handle to a node with its type erased. This allows a less safe interaction with the node, but the handle can easily be stored.
#[derive(Clone)]
pub struct UntypedHandle {
    pub(crate) node: NodeId,
    /// Allows us to send parameter changes straight to the audio thread
    sender: Arc<Mutex<SchedulingChannelProducer>>,
}
impl UntypedHandle {
    pub fn new(node: NodeId, sender: Arc<Mutex<SchedulingChannelProducer>>) -> Self {
        Self { node, sender }
    }
}

/// Handle with type data intact, without owning a T. Enables interacting with a
/// live node in a Graph, e.g. freeing and parameter changes. Allows local error
/// checking.
#[derive(Clone)]
pub struct Handle<T> {
    _phantom: PhantomData<fn(&mut T) -> ()>,
    pub(crate) untyped_handle: UntypedHandle,
}

impl<T: Gen> Handle<T> {
    pub fn new(untyped_handle: UntypedHandle) -> Self {
        Self {
            _phantom: PhantomData,
            untyped_handle,
        }
    }
    pub fn erase_type(self) -> UntypedHandle {
        self.untyped_handle
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
        value.untyped_handle.node
    }
}
pub trait HandleTrait {
    fn set<C: Into<ParameterChange>>(&self, change: C) -> Result<(), ParameterError>;
    fn from_untyped(untyped_handle: UntypedHandle) -> Self;
}
impl<T: Gen> HandleTrait for Handle<T> {
    fn set<C: Into<ParameterChange>>(&self, change: C) -> Result<(), ParameterError> {
        let c = change.into();
        let param_index = match c.param {
            knaster_core::Param::Index(param_i) => param_i,
            knaster_core::Param::Desc(desc) => {
                if let Some(param_i) = T::param_descriptions().iter().position(|d| *d == desc) {
                    param_i
                } else {
                    // Fail
                    return Err(ParameterError::DescriptionNotFound(desc));
                }
            }
        };
        // TODO: Error handling
        let mut sender = self.untyped_handle.sender.lock().unwrap();
        sender
            .push(SchedulingEvent {
                node_key: self.untyped_handle.node.key(),
                parameter: param_index,
                value: c.value,
                smoothing: c.smoothing,
                token: c.token,
                time: c.time,
            })
            .unwrap();
        Ok(())
    }

    fn from_untyped(untyped_handle: UntypedHandle) -> Self {
        Self::new(untyped_handle)
    }
}
pub trait Handleable: Sized {
    type HandleType: HandleTrait;
    fn get_handle(untyped_handle: UntypedHandle) -> Self::HandleType {
        Self::HandleType::from_untyped(untyped_handle)
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
