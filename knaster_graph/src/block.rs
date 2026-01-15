//! Raw pointer based [`Block`] types that are used in knaster_graph

use crate::core::slice;

use knaster_core::{Block, BlockRead, Float};

/// Wrapper around a raw pointer to a contiguous allocation of the size specified in
/// [`RawContiguousBlock::new`] to use it as a [`Block`].
///
/// Since the allocation is stored as a raw pointer, Rust's aliasing rules for references don't
/// apply. Several [`RawContiguousBlock`]s to the same allocation can exist at the same time, as
/// long as they are not used at the same time.
///
/// # Safety
///
/// The caller guarantees that the `buffer` pointer points to an allocation
/// at least the size of `channels * block_size` with no other mutable
/// reference to it created for the lifetime of this `RawBlock`
pub struct RawContiguousBlock<F> {
    buffer: *mut F,
    channels: usize,
    block_size: usize,
}
impl<F: Float> RawContiguousBlock<F> {
    /// Wrapper around a raw pointer to use it as a [`Block`]
    ///
    /// # Safety
    ///
    /// The caller guarantees that the `buffer` pointer points to an allocation
    /// at least the size of `channels * block_size` with no other mutable
    /// reference to it created for the lifetime of this `RawBlock`
    pub unsafe fn new(buffer: *mut F, channels: usize, block_size: usize) -> Self {
        Self {
            buffer,
            channels,
            block_size,
        }
    }
}
impl<F: Float> Block for RawContiguousBlock<F> {
    type Sample = F;

    fn channel_as_slice(&self, channel: usize) -> &[Self::Sample] {
        assert!(channel < self.channels);
        unsafe {
            slice::from_raw_parts(self.buffer.add(channel * self.block_size), self.block_size)
        }
    }

    fn channel_as_slice_mut(&mut self, channel: usize) -> &mut [Self::Sample] {
        assert!(channel < self.channels);
        unsafe {
            slice::from_raw_parts_mut(self.buffer.add(channel * self.block_size), self.block_size)
        }
    }

    fn read(&self, channel: usize, frame: usize) -> Self::Sample {
        assert!(channel < self.channels);
        assert!(frame < self.block_size);
        unsafe { *self.buffer.add(channel * self.block_size + frame) }
    }

    fn write(&mut self, value: Self::Sample, channel: usize, frame: usize) {
        assert!(channel < self.channels);
        assert!(frame < self.block_size);
        unsafe {
            (*self.buffer.add(channel * self.block_size + frame)) = value;
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn block_size(&self) -> usize {
        self.block_size
    }
}

/// Wrapper around raw pointers to use them as a [`Block`]. Each pointer is one channel which must
/// point to an allocation of at least size `block_size`.
///
/// Since the allocations are stored as raw pointers, Rust's aliasing rules for references don't
/// apply. Several [`RawAggregateBlock`]s to the same allocation can exist at the same time, as
/// long as they are not used at the same time.
///
/// # Safety
///
/// The caller guarantees that each pointer points to an allocation at least the
/// size of `block_size` with no other mutable reference to them created for the
/// lifetime of this `AggregateBlock`. The allocations pointed to also may not
/// overlap.
pub struct RawAggregateBlock<'a, F> {
    buffers: &'a [*mut F],
    block_size: usize,
}
impl<'a, F> RawAggregateBlock<'a, F> {
    /// Wrapper around raw pointers to use them as a [`Block`]. Each pointer is one channel.
    ///
    /// # Safety
    ///
    /// The caller guarantees that each pointer points to an allocation at least
    /// the size of `block_size` with no other mutable reference to them created
    /// for the lifetime of this `AggregateBlock`. The allocations pointed to
    /// also may not overlap.
    pub unsafe fn new(buffers: &'a [*mut F], block_size: usize) -> Self {
        Self {
            buffers,
            block_size,
        }
    }
}
impl<F: Float> Block for RawAggregateBlock<'_, F> {
    type Sample = F;

    fn channel_as_slice(&self, channel: usize) -> &[Self::Sample] {
        unsafe { crate::core::slice::from_raw_parts(self.buffers[channel], self.block_size) }
    }

    fn channel_as_slice_mut(&mut self, channel: usize) -> &mut [Self::Sample] {
        unsafe { crate::core::slice::from_raw_parts_mut(self.buffers[channel], self.block_size) }
    }

    fn read(&self, channel: usize, frame: usize) -> Self::Sample {
        assert!(channel < self.buffers.len());
        assert!(frame < self.block_size);
        unsafe { *self.buffers[channel].add(frame) }
    }

    fn write(&mut self, value: Self::Sample, channel: usize, frame: usize) {
        assert!(channel < self.buffers.len());
        assert!(frame < self.block_size);
        unsafe {
            (*self.buffers[channel].add(frame)) = value;
        }
    }

    fn channels(&self) -> usize {
        self.buffers.len()
    }

    fn block_size(&self) -> usize {
        self.block_size
    }
}

/// Wrapper around const raw pointers to use them as a [`Block`]. Each pointer is one channel which
/// must point to an allocation of at least size `block_size`.
///
/// Since the allocations are stored as raw pointers, Rust's aliasing rules for references don't
/// apply. Several [`RawAggregateBlockRead`]s to the same allocation can exist at the same time, as
/// long as they are not used at the same time.
/// # Safety
///
/// The caller guarantees that each pointer points to an allocation
/// at least the size of `block_size` with no other mutable
/// reference to them created for the lifetime of this `AggregateBlock`.
pub struct RawAggregateBlockRead<'a, F> {
    buffers: &'a [*const F],
    block_size: usize,
}
impl<'a, F> RawAggregateBlockRead<'a, F> {
    /// Wrapper around raw pointers to use them as a [`Block`]. Each pointer is one channel.
    ///
    /// # Safety
    ///
    /// The caller guarantees that each pointer points to an allocation
    /// at least the size of `block_size` with no other mutable
    /// reference to them created for the lifetime of this `AggregateBlock`.
    pub unsafe fn new(buffers: &'a [*const F], block_size: usize) -> Self {
        Self {
            buffers,
            block_size,
        }
    }
}
impl<F: Float> BlockRead for RawAggregateBlockRead<'_, F> {
    type Sample = F;

    fn channel_as_slice(&self, channel: usize) -> &[Self::Sample] {
        unsafe { crate::core::slice::from_raw_parts(self.buffers[channel], self.block_size) }
    }

    fn read(&self, channel: usize, frame: usize) -> Self::Sample {
        assert!(channel < self.buffers.len());
        assert!(frame < self.block_size);
        unsafe { *self.buffers[channel].add(frame) }
    }

    fn channels(&self) -> usize {
        self.buffers.len()
    }

    fn block_size(&self) -> usize {
        self.block_size
    }
}
