use crate::utils;
use core::ptr::null_mut;
use core::mem::size_of;

// uses buddy allocation

struct FreeListNode {
    next:           *mut FreeListNode,
    previous:       *mut FreeListNode,
    free_tree_index:FreeTreeIndex,
}

pub struct Allocator {
    /// OS allocates the heap
    pub heap_start: *mut u8,
    pub heap_size: usize,
    /// tracks free blocks for all sizes for fast allocation
    free_list_heads: Vec<*mut FreeListNode>,
    /// Heap is broken into a binary tree like structure.
    /// Use this to check if a tree section is free.
    free_tree:   Vec<usize>,
}

pub type BlockSize = u16;
pub type BlockLevel = u8;
pub type FreeTreeIndex = u16;

impl Allocator {
    /// uses provided ptr to manage the heap
    pub unsafe fn new(heap_start: *mut u8, heap_size: usize, block_levels: BlockLevel) -> Self {
        let block_count = 1 << (block_levels - 1);

        let block_size = (heap_size >> (block_levels - 1)) as BlockSize;
        assert!(block_size as usize >= size_of::<FreeListNode>());

        let mut free_list_heads = vec![null_mut(); block_levels as usize];
        free_list_heads[0] = heap_start as *mut FreeListNode;

        Self {
            heap_start,
            heap_size,
            free_list_heads,
            free_tree: vec![!0; utils::align_up(2 * block_count - 1, 8 * size_of::<usize>()) / 8],
        }
    }

    pub fn get_block_levels(&self) -> BlockLevel {
        self.free_list_heads.len() as BlockLevel
    }

    pub fn get_block_size(&self) -> BlockSize {
        (self.heap_size >> (self.get_block_levels() - 1)) as BlockSize
    }

    pub unsafe fn allocate(&mut self, requested_size: usize) -> (*mut u8, BlockLevel, FreeTreeIndex) {
        let mut level = 0;
        while (self.heap_size >> (level as usize + 1)) >= requested_size && level + 1 < self.get_block_levels() {
            level += 1;
        }
        let best_level = level;

        while self.free_list_heads[level as usize].is_null() && level != 0 {
            level -= 1;
        }

        if self.free_list_heads[level as usize].is_null() {
            return (null_mut(), BlockLevel::MAX, FreeTreeIndex::MAX);
        }
        assert!((*self.free_list_heads[level as usize]).previous == null_mut());

        let allocated_node = self.free_list_heads[level as usize];

        let mut left_free_tree_index = (*allocated_node).free_tree_index;
        utils::set_bit_false(&mut self.free_tree, left_free_tree_index as usize);
        if !(*allocated_node).next.is_null() {
            (*(*allocated_node).next).previous = null_mut();
        }
        self.free_list_heads[level as usize] = (*allocated_node).next;

        while best_level != level {
            level += 1;
            left_free_tree_index = (left_free_tree_index << 1) + 1;
            let to_free_node = (allocated_node as usize + (self.heap_size >> level)) as *mut FreeListNode;
            *to_free_node = FreeListNode {
                next: self.free_list_heads[level as usize],
                previous: null_mut(),
                free_tree_index: left_free_tree_index + 1,
            };
            if !self.free_list_heads[level as usize].is_null() {
                (*self.free_list_heads[level as usize]).previous = to_free_node;
            }
            self.free_list_heads[level as usize] = to_free_node;
            utils::set_bit_false(&mut self.free_tree, left_free_tree_index as usize);
        }

        (allocated_node as *mut u8, best_level, left_free_tree_index)
    }

    pub unsafe fn deallocate(&mut self, ptr: *mut u8, level: BlockLevel, free_tree_index: FreeTreeIndex) {
        utils::set_bit_true(&mut self.free_tree, free_tree_index as usize);

        let mut free_tree_index = free_tree_index;
        let mut node = ptr as *mut FreeListNode;
        let mut level = level;

        while level != 0 {
            let is_left_as_usize = (free_tree_index & 1) as usize;
            let buddy_free_tree_index = free_tree_index + 2 * is_left_as_usize as FreeTreeIndex - 1;

            if utils::get_bit(&self.free_tree, buddy_free_tree_index as usize) {
                free_tree_index = (free_tree_index - 1) >> 1;
                utils::set_bit_true(&mut self.free_tree, free_tree_index as usize);
                let block_size = self.heap_size >> level;
                // get parent node
                node = utils::align_down(node as usize, block_size << 1) as *mut FreeListNode;

                // calculating buddy node based off of parent node
                let buddy_node = (node as usize + is_left_as_usize * block_size as usize) as *mut FreeListNode;

                if !(*buddy_node).next.is_null() {
                    (*(*buddy_node).next).previous = (*buddy_node).previous;
                }
                if !(*buddy_node).previous.is_null() {
                    (*(*buddy_node).previous).next = (*buddy_node).next;
                } else {
                    self.free_list_heads[level as usize] = (*buddy_node).next;
                }
            } else {
                break;
            }
            level -= 1;
        }

        *node = FreeListNode {
            next: self.free_list_heads[level as usize],
            previous: null_mut(),
            free_tree_index,
        };
        if !self.free_list_heads[level as usize].is_null() {
            (*self.free_list_heads[level as usize]).previous = node;
        }
        self.free_list_heads[level as usize] = node;
    }
}

extern crate alloc;

// TODO: write a better test
#[test]
fn test_coalescing() {
    let heap_size = 0x4000;
    let heap_layout = unsafe { core::alloc::Layout::from_size_align_unchecked(heap_size, heap_size) };
    let heap_start = unsafe { alloc::alloc::alloc(heap_layout) };

    let mut allocator = unsafe {
        Allocator::new(heap_start, heap_size, 4)
    };

    {
        let (small0, s0, fs0) = unsafe {
            allocator.allocate(allocator.get_block_size() as usize)
        };
        let (small1, s1, fs1) = unsafe {
            allocator.allocate(allocator.get_block_size() as usize)
        };
        let (small2, s2, fs2) = unsafe {
            allocator.allocate(allocator.get_block_size() as usize)
        };
    
        unsafe {
            allocator.deallocate(small0, s0, fs0);
            allocator.deallocate(small1, s1, fs1);
        }
    
        let (medium0, _, _) = unsafe {
            allocator.allocate((allocator.get_block_size() >> 2) as usize)
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
        Allocator::new(heap_start, heap_size, 4)
    };

    {
        let (big0, b0, fb0) = unsafe {
            allocator.allocate(heap_size)
        };
        let (big1, _, _) = unsafe {
            allocator.allocate(heap_size)
        };

        assert!(big1 == null_mut());

        unsafe {
            allocator.deallocate(big0, b0, fb0);
        }
    }

    {
        for _ in 0..8 {
            unsafe { 
                allocator.allocate(heap_size / 8)
            };
        }
        let (a, _, _) = unsafe { 
            allocator.allocate(heap_size / 8)
        };
        assert!(a == null_mut());
    }
    
    unsafe {
        alloc::alloc::dealloc(allocator.heap_start, heap_layout);
    }
}