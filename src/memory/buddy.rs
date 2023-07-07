use super::*;
use crate::data_structures::bits;
use core::ptr::null_mut;

const BLOCK_LEVELS: usize = 8;
const BLOCK_COUNT: usize = 1 << (BLOCK_LEVELS - 1);
const BLOCK_SIZE: usize = HEAP_SIZE >> (BLOCK_LEVELS - 1);

const FREE_TREE_SIZE: usize = align_up(BLOCK_COUNT * 2 - 1, 64) / 64;

//TODO: using usize as *mut is dangerous, try do something more safe
struct FreeListNode {
    next:           *mut FreeListNode,
    previous:       *mut FreeListNode,
    free_tree_index:usize,
}

//TODO: rename variables
pub struct Allocator {
    /// OS allocates the heap
    heap_start: *mut u8,
    /// tracks list of all free blocks for fast allocation
    free_list_heads: [*mut FreeListNode; BLOCK_LEVELS],
    /// Heap is broken into a binary tree like structure.
    /// Use this to check if a tree section is free.
    free_tree:   [u64; FREE_TREE_SIZE],
    /// Allows to see which parts of allocated memory map to the free tree
    /// ```
    /// let block: usize;
    /// let block_index = (block - HEAP_START) / BLOCK_SIZE;
    /// let free_tree_index = block_to_free_tree[block_index];
    /// ```
    block_to_free_tree: [Option<usize>; BLOCK_COUNT],
}

impl Allocator {
    pub unsafe fn new() -> Self {
        let heap_start = alloc::alloc::alloc(HEAP_LAYOUT);
        let mut free_list_heads = [core::ptr::null_mut::<FreeListNode>(); BLOCK_LEVELS];
        free_list_heads[0] = heap_start as *mut FreeListNode;
        *(free_list_heads[0]) = FreeListNode {
            next: null_mut(),
            previous: null_mut(),
            free_tree_index: 0,
        };

        Self {
            heap_start,
            free_list_heads,
            free_tree: [0b0; FREE_TREE_SIZE],
            block_to_free_tree: [None; BLOCK_COUNT],
        }
    }

    /// `align`: must be a power of 2
    pub unsafe fn alloc(&mut self, size: usize, align: usize) -> *mut u8 {
        let size_requested = align_up(size, align);

        let mut level = 0;
        while (HEAP_SIZE >> (level + 1)) >= size_requested && level + 1 < BLOCK_LEVELS {
            level += 1;
        }
        let best_level = level;

        while level != 0 && self.free_list_heads[level].is_null() {
            level -= 1;
        }

        if self.free_list_heads[level].is_null() {
            return null_mut();
        }

        let allocated_node = self.free_list_heads[level];
        let key_ptr_to_bitmask = (allocated_node as usize - self.heap_start as usize) / BLOCK_SIZE;
        if self.block_to_free_tree[key_ptr_to_bitmask].is_some() {
            return null_mut();
        }

        let mut left_free_tree_index = (*allocated_node).free_tree_index;
        bits::set_bit_false(&mut self.free_tree, left_free_tree_index);
        if !(*allocated_node).next.is_null() {
            (*(*allocated_node).next).previous = null_mut();
        }
        self.free_list_heads[level] = (*allocated_node).next;

        while best_level != level {
            level += 1;
            left_free_tree_index = (left_free_tree_index << 1) + 1;
            let to_free_node = (allocated_node as usize + (HEAP_SIZE >> level)) as *mut FreeListNode;
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
        self.block_to_free_tree[key_ptr_to_bitmask] = Some(left_free_tree_index);

        allocated_node as *mut u8
    }

    pub unsafe fn dealloc(&mut self, ptr: *mut u8) {
        let block_index = (ptr as usize - self.heap_start as usize) / BLOCK_SIZE;
        let mut free_tree_index = self.block_to_free_tree[block_index].unwrap();

        self.block_to_free_tree[block_index] = None;
        bits::set_bit_true(&mut self.free_tree, free_tree_index);

        let mut node = ptr as *mut FreeListNode;
        let mut level = get_block_level(free_tree_index);

        while level != 0 {
            let is_left_as_usize = free_tree_index & 1;
            let buddy_free_tree_index = free_tree_index + 2 * is_left_as_usize as usize - 1;

            if bits::get_bit(&self.free_tree, buddy_free_tree_index) {
                free_tree_index = (free_tree_index - 1) >> 1;
                bits::set_bit_true(&mut self.free_tree, free_tree_index);
                let block_size = HEAP_SIZE >> level;
                node = align_down(node as usize, block_size << 1) as *mut FreeListNode;

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

impl Drop for Allocator {
    fn drop(&mut self) {
        unsafe {
            alloc::alloc::dealloc(self.heap_start, HEAP_LAYOUT);
        }
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