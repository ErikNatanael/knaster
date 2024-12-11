//! Handles allow you to interact ergonomically with a node after it has been added to a graph.
//!
//! Roadmap:
//! - Typesafe Handle types

use crate::{
    core::marker::PhantomData,
    graph::{GraphId, NodeKey},
};
use knaster_core::{
    numeric_array, typenum::Unsigned, AudioCtx, Gen, ParameterRange, ParameterValue, Parameterable,
};

#[cfg(not(feature = "std"))]
use alloc::sync::{Arc, Mutex};
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};

use crate::SchedulingChannelProducer;

/// Handle to a node with its type erased. This allows a less safe interaction with the node, but the handle can easily be stored.
#[derive(Clone)]
pub struct UntypedHandle {
    pub(crate) node: NodeKey,
    graph: GraphId,
    /// Allows us to send parameter changes straight to the audio thread
    sender: Arc<Mutex<SchedulingChannelProducer>>,
}
impl UntypedHandle {
    pub fn new(
        identifier: NodeKey,
        graph: GraphId,
        sender: Arc<Mutex<SchedulingChannelProducer>>,
    ) -> Self {
        Self {
            node: identifier,
            graph,
            sender,
        }
    }
}

// test function
pub fn give_handle<T: Gen + Handleable + Parameterable<T::Sample>>(gen: T) -> T::HandleType {
    let (tx, rx) = rtrb::RingBuffer::new(100);
    // TODO: This is just for testing
    let untyped = UntypedHandle::new(NodeKey::default(), 0, Arc::new(Mutex::new(tx)));
    <T as Handleable>::get_handle(untyped)
}
/// Handle with type data intact, without owning a T. Enables interacting with a
/// live node in a Graph, e.g. freeing and parameter changes. Allows local error
/// checking.
#[derive(Clone)]
pub struct Handle<T> {
    _phantom: PhantomData<fn(&mut T) -> ()>,
    pub(crate) untyped_handle: UntypedHandle,
}

impl<T: Gen + Parameterable<T::Sample>> Handle<T> {
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
pub trait HandleTrait {
    fn set(&mut self);
    fn from_untyped(untyped_handle: UntypedHandle) -> Self;
}
impl<T: Gen + Parameterable<T::Sample>> HandleTrait for Handle<T> {
    fn set(&mut self) {
        todo!()
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

impl<T: Gen + Parameterable<T::Sample>> Parameterable<T::Sample> for Handle<T> {
    type Parameters = T::Parameters;

    fn param_descriptions() -> numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_default_values() -> numeric_array::NumericArray<ParameterValue, Self::Parameters> {
        T::param_default_values()
    }

    fn param_range() -> numeric_array::NumericArray<ParameterRange, Self::Parameters> {
        T::param_range()
    }

    fn param_apply(&mut self, ctx: &AudioCtx, index: usize, value: ParameterValue) {
        // Instead of setting parameters directly, send changes to the scheduler
        todo!()
    }
}
