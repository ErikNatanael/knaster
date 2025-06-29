use crate::core::{marker::PhantomData, ops::Mul, slice};
use crate::{Size, float::Float};

use numeric_array::{NumericArray, typenum::Prod};
#[cfg(any(feature = "alloc", feature = "std"))]
pub use vec_block::VecBlock;

/// Trait which corresponds to some block of data with the correct sample type.
///
/// Implement this type for your own block buffer storage. E.g. you could wrap
/// an allocation given to you via FFI. If you have unique access to the
/// underlying buffer, prefer implementing [`Block`] over implementing
/// [`BlockRead`] and [`BlockReadWrite`] directly.
///
/// # Safety
///
/// Channels returned from a [`Block`] may not overlap when mutable. Doing so
/// can lead to undefined behaviour.
///
/// # Example
/// ```
/// use crate::knaster_primitives::Block;
/// use knaster_primitives::StaticBlock;
/// use knaster_primitives::typenum::*;
/// let mut block = StaticBlock::<f32, U3, U64>::new();
/// // Let's get some mutable slices for the channels
/// let mut channels = block.iter_mut();
/// let c0 = channels.next().unwrap();
/// let c1 = channels.next().unwrap();
/// let c2 = channels.next().unwrap();
/// assert!(channels.next().is_none());
/// ```
pub trait Block {
    type Sample: Float;

    fn channel_as_slice(&self, channel: usize) -> &[Self::Sample];
    /// Return a mutable slice equivalent to one channel of data.
    ///
    /// If you need multiple mutable channels concurrently, see [`Block::iter_mut`]
    ///
    /// # Safety
    ///
    /// Channels returned from a [`Block`] may not overlap. Doing so can lead to
    /// undefined behaviour.
    ///
    /// # Example
    /// ```
    /// use crate::knaster_primitives::Block;
    /// use knaster_primitives::StaticBlock;
    /// use knaster_primitives::typenum::*;
    /// let mut block = StaticBlock::<f32, U3, U64>::new();
    /// // Getting access to just one channel at a time
    /// let mut channel1 = block.channel_as_slice_mut(1);
    /// channel1[17] = 0.1;
    /// ```
    fn channel_as_slice_mut(&mut self, channel: usize) -> &mut [Self::Sample];

    /// Returns an iterator of mutable slices corresponding to all the channels of this [`Block`].
    ///
    /// # Safety
    ///
    /// The implementation of this iterator relies on the guarantee that
    /// channels returned from a [`Block`] may not overlap. If they do, calling
    /// this function is UB.
    ///
    /// # Example
    /// ```
    /// use crate::knaster_primitives::Block;
    /// use knaster_primitives::StaticBlock;
    /// use knaster_primitives::typenum::*;
    /// let mut block = StaticBlock::<f32, U3, U64>::new();
    /// // Let's get some mutable slices for the channels
    /// let mut channels = block.iter_mut();
    /// let c0: &mut [f32] = channels.next().unwrap();
    /// let c1: &mut [f32]= channels.next().unwrap();
    /// let c2: &mut [f32] = channels.next().unwrap();
    /// assert!(channels.next().is_none());
    /// ```
    /// The following will fail to compile
    /// ```compile_fail
    /// use crate::knaster_primitives::Block;
    /// use knaster_primitives::StaticBlock;
    /// use knaster_primitives::typenum::*;
    /// let mut block = StaticBlock::<f32, U3, U64>::new();
    /// let mut channels = block.iter_mut();
    /// let c0 = channels.next().unwrap();
    /// let c1 = channels.next().unwrap();
    /// let c2 = channels.next().unwrap();
    /// assert!(channels.next().is_none());
    /// // You cannot run the iterator again while the previous one is in scope.
    /// let mut channels_again = block.iter_mut();
    /// c0[0] = 1.; // (This is here to make sure `channels` can't be dropped in this example)
    /// ```
    fn iter_mut(&mut self) -> BlockIterMut<Self> {
        BlockIterMut {
            block: self,
            channels_returned: 0,
        }
    }

    /// Read one sample value in a specific channel at a specific frame in time.
    ///
    /// # Example
    /// ```
    /// use crate::knaster_primitives::Block;
    /// use knaster_primitives::StaticBlock;
    /// use knaster_primitives::typenum::*;
    /// let mut block = StaticBlock::<f32, U3, U64>::new();
    /// block.write(5.0, 2, 17);
    /// assert_eq!(block.read(2, 17), 5.0);
    /// ```
    fn read(&self, channel: usize, frame: usize) -> Self::Sample;
    /// Write one sample value in a specific channel at a specific frame in time.
    ///
    /// # Example
    /// ```
    /// use crate::knaster_primitives::Block;
    /// use knaster_primitives::StaticBlock;
    /// use knaster_primitives::typenum::*;
    /// let mut block = StaticBlock::<f64, U3, U64>::new();
    /// block.write(5.0, 2, 17);
    /// assert_eq!(block.read(2, 17), 5.0);
    /// ```
    fn write(&mut self, value: Self::Sample, channel: usize, frame: usize);

    /// The number of channels supported by this block buffer
    fn channels(&self) -> usize;
    /// The block size of each channel in the block
    fn block_size(&self) -> usize;

    /// Returns a new immutable block which starts at an offset and with virtual block size
    fn partial(
        &self,
        start_offset: usize,
        length: usize,
    ) -> PartialBlock<<Self as BlockRead>::Sample, Self>
    where
        Self: BlockRead,
    {
        PartialBlock {
            block: self,
            start_offset,
            length,
        }
    }
    /// Returns a new mutable block which starts at an offset and with virtual block size
    ///
    /// # Example
    /// ```
    /// use crate::knaster_primitives::Block;
    /// use knaster_primitives::StaticBlock;
    /// use knaster_primitives::typenum::*;
    /// let mut block = StaticBlock::<f64, U1, U64>::new();
    /// {
    ///     let mut partial_block = block.partial_mut(3, 10);
    ///     // The partial_block will now act as if it is a 10 frame long block starting at frame 3
    ///     partial_block.write(5.0, 0, 4);
    ///     assert_eq!(partial_block.channel_as_slice(0).len(), 10);
    ///     assert_eq!(partial_block.block_size(), 10);
    /// }
    /// assert_eq!(block.read(0, 4 + 3), 5.0);
    /// ```
    fn partial_mut(&mut self, start_offset: usize, length: usize) -> PartialBlockMut<Self> {
        PartialBlockMut {
            block: self,
            start_offset,
            length,
        }
    }
}
/// Iterator over the channels of a [`Block`]. See [`Block::iter_mut`]
pub struct BlockIterMut<'a, T: ?Sized> {
    block: &'a mut T,
    channels_returned: usize,
}
impl<'a, T: Block> Iterator for BlockIterMut<'a, T>
where
    T::Sample: Float,
    T::Sample: Copy,
{
    type Item = &'a mut [T::Sample];

    fn next(&mut self) -> Option<Self::Item> {
        if self.channels_returned >= self.block.channels() {
            return None;
        }
        let channel = self.block.channel_as_slice_mut(self.channels_returned);
        // Erase the lifetime coupling to &mut self. The lifetime only needs to
        // be coupled to the original block, 'a. This relies on the channels never overlapping.
        let channel = unsafe {
            let len = channel.len();
            let ptr = channel.as_mut_ptr();
            slice::from_raw_parts_mut(ptr, len)
        };
        self.channels_returned += 1;
        Some(channel)
    }
}
/// Subset of [`Block`] with read only access to buffer data.
///
/// This trait can be implemented for a wrapper around a read-only buffer only to be used as an
/// input for processing, never as an output, e.g. a `&[T]`
pub trait BlockRead {
    type Sample: Float;

    /// Return an immutable slice of the samples in a specific channel.
    fn channel_as_slice(&self, channel: usize) -> &[Self::Sample];

    /// Return the sample at a specific frame in a specific channel.
    fn read(&self, channel: usize, frame: usize) -> Self::Sample;

    /// The number of channels supported by this block buffer
    fn channels(&self) -> usize;
    /// The block size of each channel in the block
    fn block_size(&self) -> usize;

    /// Returns a new immutable block which starts at an offset and with virtual block size
    fn partial(&self, start_offset: usize, length: usize) -> PartialBlock<Self::Sample, Self> {
        PartialBlock {
            block: self,
            start_offset,
            length,
        }
    }
}
impl<T: Block> BlockRead for &T {
    type Sample = T::Sample;

    fn channel_as_slice(&self, channel: usize) -> &[Self::Sample] {
        T::channel_as_slice(self, channel)
    }

    fn read(&self, channel: usize, frame: usize) -> Self::Sample {
        T::read(self, channel, frame)
    }

    fn channels(&self) -> usize {
        T::channels(self)
    }

    fn block_size(&self) -> usize {
        T::block_size(self)
    }
}

/// A block which is a subset of another (immutable) block.
///
/// This is used to implement the [`BlockRead`] trait for a block which is a subset of another
/// block, e.g. when doing partial block processing with parameter changes in the middle.
pub struct PartialBlock<'a, F, T: 'a + BlockRead<Sample = F> + ?Sized> {
    block: &'a T,
    start_offset: usize,
    length: usize,
}
impl<'a, F: Float, T: 'a + BlockRead<Sample = F>> BlockRead for PartialBlock<'a, F, T> {
    type Sample = F;

    fn channel_as_slice(&self, channel: usize) -> &[Self::Sample] {
        &self.block.channel_as_slice(channel)[self.start_offset..(self.start_offset + self.length)]
    }

    fn read(&self, channel: usize, frame: usize) -> Self::Sample {
        self.block.read(channel, frame + self.start_offset)
    }

    fn channels(&self) -> usize {
        self.block.channels()
    }

    fn block_size(&self) -> usize {
        self.length
    }
}
/// A block which is a subset of another mutable block.
///
/// This is used to implement the [`Block`] trait for a block which is a subset of another
/// block, e.g. when doing partial block processing with parameter changes in the middle.
pub struct PartialBlockMut<'a, T: Block + ?Sized> {
    block: &'a mut T,
    start_offset: usize,
    length: usize,
}
impl<T: Block> Block for PartialBlockMut<'_, T> {
    type Sample = T::Sample;

    fn channel_as_slice(&self, channel: usize) -> &[Self::Sample] {
        &self.block.channel_as_slice(channel)[self.start_offset..(self.start_offset + self.length)]
    }

    fn channel_as_slice_mut(&mut self, channel: usize) -> &mut [Self::Sample] {
        &mut self.block.channel_as_slice_mut(channel)
            [self.start_offset..(self.start_offset + self.length)]
    }

    fn read(&self, channel: usize, frame: usize) -> Self::Sample {
        self.block.read(channel, frame + self.start_offset)
    }

    fn write(&mut self, value: Self::Sample, channel: usize, frame: usize) {
        self.block.write(value, channel, frame + self.start_offset);
    }

    fn channels(&self) -> usize {
        self.block.channels()
    }

    fn block_size(&self) -> usize {
        self.length
    }
}
/// Iterator over the channels of a [`PartialBlockMut`].
pub struct PartialBlockIterkMut<'a, T: Block + ?Sized> {
    block: &'a mut T,
    start_offset: usize,
    length: usize,
    channels_returned: usize,
}
impl<'b, T: Block + ?Sized> Iterator for PartialBlockIterkMut<'b, T>
where
    T::Sample: Copy,
{
    type Item = &'b mut [T::Sample];

    fn next(&mut self) -> Option<Self::Item> {
        let channel = self.block.channel_as_slice_mut(self.channels_returned);
        // This has the effect of erasing the lifetime of the slice,
        // as well as making it a partial channel instead of a the full channel.
        let channel = unsafe {
            let ptr = channel.as_mut_ptr();
            slice::from_raw_parts_mut(ptr.add(self.start_offset), self.length)
        };
        self.channels_returned += 1;
        Some(channel)
    }
}

/// A block which has no data in it, i.e. there are zero channels.
pub struct EmptyBlock<F>(PhantomData<F>);
impl<F: Float> Default for EmptyBlock<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Float> EmptyBlock<F> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}
impl<F: Float> Block for EmptyBlock<F> {
    type Sample = F;

    fn channel_as_slice(&self, _channel: usize) -> &[Self::Sample] {
        unreachable!()
    }

    fn channel_as_slice_mut(&mut self, _channel: usize) -> &mut [Self::Sample] {
        unreachable!()
    }

    fn read(&self, _channel: usize, _frame: usize) -> Self::Sample {
        unreachable!()
    }

    fn write(&mut self, _value: Self::Sample, _channel: usize, _frame: usize) {
        unreachable!()
    }

    fn channels(&self) -> usize {
        0
    }

    fn block_size(&self) -> usize {
        0
    }
}

pub fn empty_block<F: Float>() -> EmptyBlock<F> {
    EmptyBlock::new()
}

#[cfg(any(feature = "alloc", feature = "std"))]
mod vec_block {
    use super::Block;
    use crate::Float;
    #[cfg(all(feature = "alloc", not(feature = "std")))]
    use alloc::vec;
    #[cfg(all(feature = "alloc", not(feature = "std")))]
    use alloc::vec::Vec;

    #[cfg(feature = "std")]
    use std::vec;
    #[cfg(feature = "std")]
    use std::vec::Vec;

    /// A Block backed by a Vec heap allocation
    pub struct VecBlock<F> {
        buffer: Vec<F>,
        channels: usize,
        block_size: usize,
    }
    impl<F: Float> VecBlock<F> {
        pub fn new(channels: usize, block_size: usize) -> Self {
            Self {
                buffer: vec![F::ZERO; channels * block_size],
                channels,
                block_size,
            }
        }
        pub unsafe fn ptr_to_internal_buffer(&self) -> *const F {
            self.buffer.as_ptr()
        }
    }
    impl<F: Float> Block for VecBlock<F> {
        type Sample = F;

        fn channel_as_slice(&self, channel: usize) -> &[Self::Sample] {
            assert!(channel < self.channels);
            &self.buffer[(channel * self.block_size)..((channel + 1) * self.block_size)]
        }

        fn channel_as_slice_mut(&mut self, channel: usize) -> &mut [Self::Sample] {
            &mut self.buffer[(channel * self.block_size)..((channel + 1) * self.block_size)]
        }

        fn read(&self, channel: usize, frame: usize) -> Self::Sample {
            self.buffer[channel * self.block_size + frame]
        }

        fn write(&mut self, value: Self::Sample, channel: usize, frame: usize) {
            self.buffer[channel * self.block_size + frame] = value;
        }

        fn channels(&self) -> usize {
            self.channels
        }

        fn block_size(&self) -> usize {
            self.block_size
        }
    }
}

/// A stack allocated block for when the block size and the channel count
/// necessary for a block is known at compile time.
// Why not const generic usizes? You can't do maths on them on stable Rust at the time of writing.
#[allow(non_camel_case_types)]
pub struct StaticBlock<F, CHANNELS: Size, BLOCK_SIZE: Size>
where
    BLOCK_SIZE: Mul<CHANNELS>,
    Prod<BLOCK_SIZE, CHANNELS>: Size,
{
    buffer: NumericArray<F, Prod<BLOCK_SIZE, CHANNELS>>,
}
#[allow(non_camel_case_types)]
impl<BLOCK_SIZE: Size, CHANNELS: Size, F: Float> StaticBlock<F, CHANNELS, BLOCK_SIZE>
where
    BLOCK_SIZE: Mul<CHANNELS>,
    Prod<BLOCK_SIZE, CHANNELS>: Size,
{
    pub fn new() -> Self {
        Self {
            buffer: NumericArray::default(),
        }
    }
}

impl<BLOCK_SIZE: Size, CHANNELS: Size, F: Float> Default for StaticBlock<F, CHANNELS, BLOCK_SIZE>
where
    BLOCK_SIZE: Mul<CHANNELS>,
    Prod<BLOCK_SIZE, CHANNELS>: Size,
{
    fn default() -> Self {
        Self::new()
    }
}
#[allow(non_camel_case_types)]
impl<BLOCK_SIZE: Size, CHANNELS: Size, F: Float> Block for StaticBlock<F, CHANNELS, BLOCK_SIZE>
where
    BLOCK_SIZE: Mul<CHANNELS>,
    Prod<BLOCK_SIZE, CHANNELS>: Size,
{
    type Sample = F;

    fn channel_as_slice(&self, channel: usize) -> &[Self::Sample] {
        &self.buffer[(channel * BLOCK_SIZE::USIZE)..((channel + 1) * BLOCK_SIZE::USIZE)]
    }

    fn channel_as_slice_mut(&mut self, channel: usize) -> &mut [Self::Sample] {
        &mut self.buffer[(channel * BLOCK_SIZE::USIZE)..((channel + 1) * BLOCK_SIZE::USIZE)]
    }

    fn read(&self, channel: usize, frame: usize) -> Self::Sample {
        self.buffer[(channel * BLOCK_SIZE::USIZE) + frame]
    }

    fn write(&mut self, value: Self::Sample, channel: usize, frame: usize) {
        self.buffer[(channel * BLOCK_SIZE::USIZE) + frame] = value;
    }

    fn channels(&self) -> usize {
        CHANNELS::USIZE
    }

    fn block_size(&self) -> usize {
        BLOCK_SIZE::USIZE
    }
}

#[allow(non_camel_case_types)]
impl<BLOCK_SIZE: Size, CHANNELS: Size, F: Float> BlockRead for StaticBlock<F, CHANNELS, BLOCK_SIZE>
where
    BLOCK_SIZE: Mul<CHANNELS>,
    Prod<BLOCK_SIZE, CHANNELS>: Size,
{
    type Sample = F;

    fn channel_as_slice(&self, channel: usize) -> &[Self::Sample] {
        Block::channel_as_slice(self, channel)
    }

    fn read(&self, channel: usize, frame: usize) -> Self::Sample {
        Block::read(self, channel, frame)
    }

    fn channels(&self) -> usize {
        Block::channels(self)
    }

    fn block_size(&self) -> usize {
        Block::block_size(self)
    }
}
