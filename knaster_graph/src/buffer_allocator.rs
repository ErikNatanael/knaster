//! A simple allocator for the Graph
//!
//! Allocation happens in two steps. First, buffers are requested and given as
//! usize offsets into an allocation. When all blocks have been assigned, new
//! memory is allocated if necessary and the offsets are converted into real
//! pointers to memory.
//!
//! # How to use
//! ```rust
//! let a = BufferAllocator::<f32>::new(128);
//! // 1. Assign blocks
//! let in_block = a.get_block(64);
//! let out_block = a.get_block(64);
//! // We pretend the node has been processed
//! // The in_block_can be returned
//! a.return_block(in_block);
//! // The old out_block becomes the input block to the next 2 Gens
//! let out_block2 = a.get_block(64);
//! let out_block3 = a.get_block(64);
//! a.return_block(out_block);
//! a.return_block(out_block2);
//! a.return_block(out_block3);
//! // Let the allocator reallocate if necessary
//! a.finished_assigning_make_allocation();
//! // Replace the offsets by ptrs
//! let in_block = a.offset_to_ptr(in_block).unwrap();
//! let out_block = a.offset_to_ptr(out_block).unwrap();
//! let out_block2 = a.offset_to_ptr(out_block2).unwrap();
//! let out_block3 = a.offset_to_ptr(out_block3).unwrap();
//! ```
//!

use crate::core::sync::Arc;
use crate::graph::OwnedRawBuffer;
use knaster_core::Float;

struct AllocatedBlock {
    start_offset: usize,
    len: usize,
    // The number of channels in nodes that are borrowing from this block. When
    // they have returned their borrows the block can be reused. We are counting
    // channels because that's how the input edges are stored.
    outstanding_borrows: usize,
}

pub(crate) struct BufferAllocator<F: Float> {
    buffer: Arc<OwnedRawBuffer<F>>,
    next_free_pos: usize,
    // When a block no longer is being used it can be returned and then reused
    allocated_blocks: Vec<AllocatedBlock>,
    return_order: Vec<usize>,
    virtual_allocation_size: usize,
}
impl<F: Float> BufferAllocator<F> {
    pub fn new(len: usize) -> Self {
        Self {
            buffer: Arc::new(OwnedRawBuffer::new(len)),
            next_free_pos: 0,
            allocated_blocks: Vec::new(),
            return_order: Vec::new(),
            virtual_allocation_size: 0,
        }
    }
    /// Returns a clone of the Arc for keeping this allocation alive
    pub fn buffer(&self) -> Arc<OwnedRawBuffer<F>> {
        self.buffer.clone()
    }
    pub fn assign_new_block(
        &mut self,
        num_channels: usize,
        block_size: usize,
        num_borrows: usize,
    ) -> usize {
        if self.next_free_pos + num_channels * block_size > self.virtual_allocation_size {
            self.virtual_allocation_size = self.next_free_pos + num_channels * block_size;
        }
        let start_offset = self.next_free_pos;
        self.allocated_blocks.push(AllocatedBlock {
            start_offset,
            len: num_channels * block_size,
            outstanding_borrows: num_borrows,
        });
        self.next_free_pos += num_channels * block_size;
        start_offset
    }
    // "Free" the block to be used by another node later in the tree.
    pub fn return_block(&mut self, start_offset: usize) {
        for (i, block) in self.allocated_blocks.iter_mut().enumerate() {
            if block.start_offset == start_offset {
                block.outstanding_borrows -= 1;
                if block.outstanding_borrows == 0 {
                    self.return_order.push(i);
                }
                break;
            }
        }
    }
    pub fn empty_channel(&self) -> *const F {
        self.buffer.add(0).unwrap().cast_const()
    }
    // Returns the start offset of a block of the given size valid until returned
    pub fn get_block(
        &mut self,
        num_channels: usize,
        block_size: usize,
        num_borrows: usize,
    ) -> usize {
        let len = num_channels * block_size;
        // 1. Try to use an existing allocated block in order of return. If only
        // part of a block is needed, split it.
        for i in (self.return_order.len() - 1)..=0 {
            let index = self.return_order[i];
            if self.allocated_blocks[index].outstanding_borrows == 0
                && self.allocated_blocks[index].len <= len
            {
                // It's a match!
                self.allocated_blocks[index].outstanding_borrows = num_borrows;
                self.return_order.remove(i);
                return self.allocated_blocks[index].start_offset;
            }
        }
        // 2. If blocks are available, but too small, try to merge adjacent ones
        // to fit the larger allocation. Merged blocks are still separate
        // entries in `allocated_blocks`, but multiple in a row are.

        // todo!();

        // 3. Allocate a new block
        self.assign_new_block(num_channels, block_size, num_borrows)
    }
    pub fn reset(&mut self, block_size: usize) {
        self.next_free_pos = 0;
        self.virtual_allocation_size = 0;
        self.allocated_blocks.clear();
        self.return_order.clear();
        // Allocate one empty channel, always at the start, for assigning to missing inputs
        // TODO: usize::MAX is not a beautiful way of making sure the block is never reused/always available
        self.get_block(1, block_size, usize::MAX);
    }
    /// Call this when all blocks have been assigned. If a new allocation needs
    /// to be made, the old one is returned to be dropped once the other Arc is
    /// dropped.
    #[must_use]
    pub fn finished_assigning_make_allocation(&mut self) -> Option<Arc<OwnedRawBuffer<F>>> {
        if self.virtual_allocation_size <= self.buffer.ptr.len() {
            // The allocation is sufficiently large, we can reuse it.
            return None;
        }
        // We need to reallocate
        let buffer = Arc::new(OwnedRawBuffer::new(self.virtual_allocation_size));
        Some(crate::core::mem::replace(&mut self.buffer, buffer))
    }
    /// Exchange the allocation offset value from earlier by a pointer value to
    /// a potentially new allocation.
    ///
    /// Run this after `finished_assigning_make_allocation`. If used correctly,
    /// it should always return a Some.
    pub fn offset_to_ptr(&self, offset: usize) -> Option<*mut F> {
        self.buffer.add(offset)
    }
}
