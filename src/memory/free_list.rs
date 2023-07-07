use core::mem::{size_of, align_of};
use core::ptr::null_mut;

use crate::memory::*;

// tries allocate from the end of the free list node
// so we can simply shrink the size
struct FreeListNode {
    /// use effective size for allocations
    size: usize,
    /// (*next).previous never null
    next: *mut FreeListNode,
    /// (*previous).next never null
    previous: *mut FreeListNode,
}

unsafe fn start_addr(node: *mut FreeListNode) -> *mut FreeListNode {
    node 
}

unsafe fn end_addr(node: *mut FreeListNode) -> *mut FreeListNode {
    (node as usize + (*node).size) as *mut FreeListNode
}

unsafe fn effective_start_ptr(node: *mut FreeListNode, align: usize) -> *mut u8 {
    align_ptr_up(node as *mut u8, align)
}

unsafe fn effective_end_ptr(node: *mut FreeListNode, align: usize) -> *mut u8 {
    align_ptr_down((node as usize + (*node).size) as *mut u8, align)
}

pub struct FreeListAllocator {
    heap_start: *mut u8,
    heap_size: usize,

    free_list_head: *mut FreeListNode,
    // array for quick retrival of free list nodes
    // free_list_nodes: *mut *mut FreeListNode,
}

impl FreeListAllocator {
    pub unsafe fn new(heap_size: usize) -> Self {
        let heap_size = align_up(heap_size, align_of::<FreeListNode>());
        let heap_start = alloc::alloc::alloc(
            core::alloc::Layout::from_size_align_unchecked(heap_size, align_of::<FreeListNode>())
        );

        let free_list_head = heap_start as *mut FreeListNode;
        *free_list_head = FreeListNode {
            size: heap_size,
            next: null_mut(),
            previous: null_mut(),
        };

        // let free_array_capacity = heap_size / size_of::<FreeListNode>();
        // let free_array_size = free_array_capacity * size_of::<FreeListNode>();

        // // TODO: call alloc once
        // let free_list_nodes = alloc::alloc::alloc(
        //     core::alloc::Layout::from_size_align_unchecked(free_array_size, align_of::<FreeListNode>())
        // ) as *mut *mut FreeListNode;
        // *free_list_nodes = heap_start as *mut FreeListNode;

        // for i in 1..free_array_capacity {
        //     *((free_list_nodes as usize + i * size_of::<*mut FreeListNode>()) as *mut *mut FreeListNode) = null_mut();
        // }

        Self {
            heap_start,
            heap_size,

            free_list_head: heap_start as *mut FreeListNode,
            // free_list_nodes,
        }
    }

    unsafe fn invalidate_node(node: *mut FreeListNode) {
        (*node).previous = null_mut();
        (*node).next = null_mut();
        (*node).size = 0;
    }

    /// `node`: must not be null
    unsafe fn remove_node(&mut self, node: *mut FreeListNode) {
        let next = (*node).next;
        let previous = (*node).previous;
        
        if previous.is_null() {
            self.free_list_head = null_mut();
        } else {
            (*previous).next = next;
        }
        if !next.is_null() {
            (*next).previous = previous;
        }

        // for safety precautions!
        Self::invalidate_node(node);
    }
}

impl Allocator for FreeListAllocator {
    // TODO: fix alignment issues for the freelist node allocation
    unsafe fn alloc(&mut self, requested_size: usize, requested_align: usize) -> (*mut u8, usize) {
        
        let mut node = self.free_list_head;
        assert!((*node).previous == null_mut());

        while !node.is_null() {
            let effective_start_ptr = effective_start_ptr(node, requested_align);
            let effective_end_ptr = effective_end_ptr(node, requested_align);
            let effective_size = effective_end_ptr as isize - effective_start_ptr as isize;

            if effective_size >= requested_size as isize {
                let effective_size = effective_size as usize;

                if effective_start_ptr as usize - node as usize >= size_of::<FreeListNode>() {
                    assert!(effective_size > requested_size);
                    (*node).size = effective_start_ptr as usize - node as usize;

                    if effective_size - requested_size >= size_of::<FreeListNode>() {
                        (*node).size += effective_size - requested_size;
                    }
                } else {
                    

                }

                let node_end_ptr = end_addr(node);
                if node_end_ptr as usize - effective_end_ptr as usize >= size_of::<FreeListNode>() {
                    let new_node = node_end_ptr;
                    (*new_node).size = node_end_ptr as usize - effective_end_ptr as usize;

                    if (*node).size >= size_of::<FreeListNode>() {
                        (*new_node).previous = node;
                        (*new_node).next = (*node).next;
                        if !((*node).next).is_null() {
                            (*((*node).next)).previous = new_node;
                        }
                        (*node).next = new_node;
                    } else {
                        (*new_node).previous = (*node).previous;
                        if ((*node).previous).is_null() {
                            self.free_list_head = new_node;
                        } else {
                            (*((*node).previous)).next = new_node;
                        }
                        (*new_node).next = (*node).next;
                        if !((*node).next).is_null() {
                            (*((*node).next)).previous = new_node;
                        }

                        // safety
                        (*node).next = null_mut();
                        (*node).previous = null_mut();
                    }
                }

                return (
                    ( effective_start_ptr as usize + effective_size - requested_size) as *mut u8,
                    effective_size,
                );
            }
            

            node = (*node).next;
        }

        (null_mut(), 0)
    }

    unsafe fn dealloc(&mut self, allocated_ptr: *mut u8) {
        todo!()
    }
}

impl Drop for FreeListAllocator {
    fn drop(&mut self) {
        todo!()
    }
}