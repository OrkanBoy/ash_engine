use super::*;
use crate::data_structures::bits;
use core::ptr::null_mut;
use core::mem::{size_of, align_of};

//TODO: using *mut FreeListNode is dangerous, try do something more safe
struct FreeListNode {
    next:           *mut FreeListNode,
    previous:       *mut FreeListNode,
    free_tree_index:usize,
}

// TODO: ideally do not use Vec, rather another allocator
pub struct BuddyAllocator {
    /// OS allocates the heap
    pub heap_start: *mut u8,
    pub heap_size: usize,
    /// smallest size of a single block
    block_size: usize,
    /// tracks free blocks for all sizes for fast allocation
    free_list_heads: Vec<*mut FreeListNode>,
    /// Heap is broken into a binary tree like structure.
    /// Use this to check if a tree section is free.
    free_tree:   Vec<usize>,
    /// Allows to see which parts of allocated memory map to the free tree
    /// ```
    /// let block: usize;
    /// let block_index = (block - HEAP_START) / BLOCK_SIZE;
    /// let free_tree_index = block_to_free_tree[block_index];
    /// ```
    block_to_free_tree: Vec<Option<usize>>,
}

impl BuddyAllocator {
    /// uses provided ptr to manage the heap
    pub unsafe fn new(heap_start: *mut u8, heap_size: usize, block_levels: usize) -> Self {
        let block_count = 1 << (block_levels - 1);
        let block_size = heap_size >> (block_levels - 1);
        assert!(block_size >= size_of::<FreeListNode>());

        let mut free_list_heads = vec![null_mut(); block_levels];
        free_list_heads[0] = heap_start as *mut FreeListNode;

        Self {
            heap_start,
            heap_size,
            free_list_heads,
            block_size,
            free_tree: vec![!0; align_up(2 * block_count - 1, 8 * size_of::<usize>()) / 8],
            block_to_free_tree: vec![None; block_count],
        }
    }

    pub fn get_block_count(&self) -> usize {
        self.block_to_free_tree.len()
    }

    pub fn get_block_levels(&self) -> usize {
        self.free_list_heads.len()
    }
}

impl Allocator for BuddyAllocator {
    unsafe fn alloc(&mut self, requested_size: usize, _requested_align: usize) -> (*mut u8, usize) {
        let mut level = 0;
        while (self.heap_size >> (level + 1)) >= requested_size && level + 1 < self.get_block_levels() {
            level += 1;
        }
        let best_level = level;

        while self.free_list_heads[level].is_null() && level != 0 {
            level -= 1;
        }

        if self.free_list_heads[level].is_null() {
            log::warn!("Failed to allocate");
            return (null_mut(), 0);
        }
        assert!((*self.free_list_heads[level]).previous == null_mut());

        let allocated_node = self.free_list_heads[level];
        let block_index = (allocated_node as usize - self.heap_start as usize) / self.block_size;

        assert!(self.block_to_free_tree[block_index].is_none());

        let mut left_free_tree_index = (*allocated_node).free_tree_index;
        bits::set_bit_false(&mut self.free_tree, left_free_tree_index);
        if !(*allocated_node).next.is_null() {
            (*(*allocated_node).next).previous = null_mut();
        }
        self.free_list_heads[level] = (*allocated_node).next;

        while best_level != level {
            level += 1;
            left_free_tree_index = (left_free_tree_index << 1) + 1;
            let to_free_node = (allocated_node as usize + (self.heap_size >> level)) as *mut FreeListNode;
            *to_free_node = FreeListNode {
                next: self.free_list_heads[level],
                previous: null_mut(),
                free_tree_index: left_free_tree_index + 1,
            };
            if !self.free_list_heads[level].is_null() {
                (*self.free_list_heads[level]).previous = to_free_node;
            }
            self.free_list_heads[level] = to_free_node;
            bits::set_bit_false(&mut self.free_tree, left_free_tree_index);
        }
        self.block_to_free_tree[block_index] = Some(left_free_tree_index);

        (allocated_node as *mut u8, self.heap_size >> level)
    }

    unsafe fn dealloc(&mut self, allocated_ptr: *mut u8) {
        let block_index = (allocated_ptr as usize - self.heap_start as usize) / self.block_size;
        let mut free_tree_index = self.block_to_free_tree[block_index].unwrap();

        self.block_to_free_tree[block_index] = None;
        bits::set_bit_true(&mut self.free_tree, free_tree_index);

        let mut node = allocated_ptr as *mut FreeListNode;
        let mut level = get_block_level(free_tree_index);

        while level != 0 {
            let is_left_as_usize = free_tree_index & 1;
            let buddy_free_tree_index = free_tree_index + 2 * is_left_as_usize as usize - 1;

            if bits::get_bit(&self.free_tree, buddy_free_tree_index) {
                free_tree_index = (free_tree_index - 1) >> 1;
                bits::set_bit_true(&mut self.free_tree, free_tree_index);
                let block_size = self.heap_size >> level;
                // get parent node
                node = align_down(node as usize, block_size << 1) as *mut FreeListNode;

                // calculating buddy node based off of parent node
                let buddy_node = (node as usize + is_left_as_usize * block_size) as *mut FreeListNode;

                if !(*buddy_node).next.is_null() {
                    (*(*buddy_node).next).previous = (*buddy_node).previous;
                }
                if !(*buddy_node).previous.is_null() {
                    (*(*buddy_node).previous).next = (*buddy_node).next;
                } else {
                    self.free_list_heads[level] = (*buddy_node).next;
                }
            } else {
                break;
            }
            level -= 1;
        }

        *node = FreeListNode {
            next: self.free_list_heads[level],
            previous: null_mut(),
            free_tree_index,
        };
        if !self.free_list_heads[level].is_null() {
            (*self.free_list_heads[level]).previous = node;
        }
        self.free_list_heads[level] = node;
    }
}

fn get_block_level(free_tree_index: usize) -> usize {
    let free_tree_index = (free_tree_index + 1) >> 1;
    let mut level = 0;
    while 1 << level <= free_tree_index  {
        level += 1;
    }
    level
}

#[test]
fn test_get_block_level() {
    let block_levels = 8;
    let block_count = 1 << (block_levels - 1);

    let index_to_level = {
        let mut index_to_level = Vec::with_capacity(2 * block_count - 1);

        let mut index = 0_usize;
        for level in 0..block_levels {
            for _ in 0..1 << level {
                index_to_level.push((index, level));
                index += 1;
            }
        }
        index_to_level
    };

    for (index, level) in index_to_level {
        assert!(get_block_level(index) == level);
    }
}

// TODO: write a better test
#[test]
fn test_coalescing() {
    let heap_size = 0x4000;
    let heap_layout = unsafe { core::alloc::Layout::from_size_align_unchecked(heap_size, heap_size) };
    let heap_start = unsafe { alloc::alloc::alloc(heap_layout) };

    let mut allocator = unsafe {
        BuddyAllocator::new(heap_start, heap_size, 4)
    };

    {
        let (small0, _) = unsafe {
            allocator.alloc(allocator.block_size, 1)
        };
        let (small1, _) = unsafe {
            allocator.alloc(allocator.block_size, 1)
        };
        let (small2, _) = unsafe {
            allocator.alloc(allocator.block_size, 1)
        };
    
        unsafe {
            allocator.dealloc(small0);
            allocator.dealloc(small1);
        }
    
        let (medium0, _) = unsafe {
            allocator.alloc(allocator.block_size >> 2, 1)
        };
    
        assert!(small0 != medium0);
    }
    
    unsafe {
        alloc::alloc::dealloc(allocator.heap_start, heap_layout);
    }
}

#[test]
fn test_failed_allocation() {
    let heap_size = 0x4000;
    let heap_layout = unsafe { core::alloc::Layout::from_size_align_unchecked(heap_size, heap_size) };
    let heap_start = unsafe { alloc::alloc::alloc(heap_layout) };

    let mut allocator = unsafe {
        BuddyAllocator::new(heap_start, heap_size, 4)
    };

    {
        let (big0, _) = unsafe {
            allocator.alloc(heap_size, 1)
        };
        let (big1, _) = unsafe {
            allocator.alloc(heap_size, 1)
        };

        assert!(big1 == null_mut());

        unsafe {
            allocator.dealloc(big0);
        }
    }

    {
        for _ in 0..8 {
            let (_, s) = unsafe { 
                allocator.alloc(heap_size / 8, 1)
            };
            println!("{s}");
            assert!(s == heap_size / 8);
        }
        let (_, s) = unsafe { 
            allocator.alloc(heap_size / 8, 1)
        };
        assert!(s == 0);
    }
    
    unsafe {
        alloc::alloc::dealloc(allocator.heap_start, heap_layout);
    }
}