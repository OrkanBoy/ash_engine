pub struct Vertex {
    pub x: f32, pub y: f32, pub z: f32,
    
    pub u: f32, pub v: f32,
}

use std::rc::Rc;
use core::mem::size_of;
use crate::{allocator, utils};

use ash::vk;

type GeometryId = u16;
type Index = u32;

// TODO: configurable
const VK_INDEX_TYPE: vk::IndexType = vk::IndexType::UINT32;

/// referred by a geometry id from user and used internally for binding that geometry.
/// A slice of these is used to quickly iterate and call vkCmdDrawIndexed.
/// They are also used to deallocate the underlying geometry
#[derive(Clone)]
struct Geometry {
    vertex_offset:  i32,
    first_index:    u32,
    index_count:    u32,
}

impl Default for Geometry {
    fn default() -> Self {
        Self { vertex_offset: i32::MIN, first_index: 0, index_count: 0 }
    }
}

#[derive(Clone)]
struct GeometryDealloc {
    vertex_block_level:     allocator::BlockLevel,
    vertex_free_tree_index: allocator::FreeTreeIndex,
    index_block_level:      allocator::BlockLevel,
    index_free_tree_index:  allocator::FreeTreeIndex,
}

impl Default for GeometryDealloc {
    fn default() -> Self {
        use allocator::{BlockLevel, FreeTreeIndex};

        Self { 
            vertex_block_level:     BlockLevel::MAX,
            vertex_free_tree_index: FreeTreeIndex::MAX,
            index_block_level:      BlockLevel::MAX,
            index_free_tree_index:  FreeTreeIndex::MAX,
        }
    } 
}

/// Holds static geometry. 
/// User provides vertex and index data and system loads data onto device local memory
/// System also returns back geometry id which refers to the loaded geometry
pub struct GeometrySystem {
    device:                     Rc<ash::Device>,
    due_vertex_buffer_copies:   Vec<vk::BufferCopy>,
    due_index_buffer_copies:    Vec<vk::BufferCopy>,
    
    id_to_geometry:             Vec<Geometry>,
    id_to_geometry_dealloc:     Vec<GeometryDealloc>,
    id_exists:                  Vec<usize>,
    available_ids:              Vec<GeometryId>,
    geometry_count:             usize,

    vertex_buffer:              vk::Buffer,
    index_buffer:               vk::Buffer,
    memory:                     vk::DeviceMemory,
    
    staging_buffer:             vk::Buffer,
    staging_memory:             vk::DeviceMemory,

    vertex_allocator:           allocator::Allocator,
    index_allocator:            allocator::Allocator,
}

impl GeometrySystem {
    pub fn new(
        device: Rc<ash::Device>, 
        physical_device_memory_properties: &vk::PhysicalDeviceMemoryProperties, 
        vertex_buffer_size: vk::DeviceSize,
        index_buffer_size: vk::DeviceSize,
    ) -> Self {
        let vertex_buffer = {
            let info = vk::BufferCreateInfo::builder()
                .size(vertex_buffer_size)
                .usage(vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE); // configurable
            unsafe { device.create_buffer(&info, None) }.expect("Failed to create buffer handle")
        };

        let index_buffer = {
            let info = vk::BufferCreateInfo::builder()
                .size(index_buffer_size)
                .usage(vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE); // configurable
            unsafe { device.create_buffer(&info, None) }.expect("Failed to create buffer handle")
        };

        let staging_buffer = {
            let info = vk::BufferCreateInfo::builder()
                .size(vertex_buffer_size + index_buffer_size)
                .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                .sharing_mode(vk::SharingMode::EXCLUSIVE); // configurable
            unsafe { device.create_buffer(&info, None) }.expect("Failed to create buffer handle")
        };

        let vertex_mem_requirements = unsafe { device.get_buffer_memory_requirements(vertex_buffer) };
        let index_mem_requirements = unsafe { device.get_buffer_memory_requirements(index_buffer) };
        let staging_mem_requirements = unsafe { device.get_buffer_memory_requirements(staging_buffer) };

        let memory = {
            let mem_type_index = crate::renderer::device::find_mem_type_index(
                vertex_mem_requirements.memory_type_bits & index_mem_requirements.memory_type_bits,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                &physical_device_memory_properties,
            );
            let alloc_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(vertex_mem_requirements.size + index_mem_requirements.size)
                .memory_type_index(mem_type_index);

            unsafe { device.allocate_memory(&alloc_info, None) }
                .expect("Failed to allocate device memory")
        };

        let staging_memory = {
            let mem_type_index = crate::renderer::device::find_mem_type_index(
                staging_mem_requirements.memory_type_bits,
                // TODO: optimize with host caches and memory flushes
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT, 
                &physical_device_memory_properties,
            );
            let alloc_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(staging_mem_requirements.size)
                .memory_type_index(mem_type_index);

            unsafe { device.allocate_memory(&alloc_info, None) }
                .expect("Failed to allocate device memory")
        };

        unsafe {
            let bind_infos = [
                vk::BindBufferMemoryInfo::builder()
                    .memory(memory)
                    .memory_offset(0)
                    .buffer(vertex_buffer)
                    .build(),
                vk::BindBufferMemoryInfo::builder()
                    .memory(memory)
                    .memory_offset(vertex_buffer_size)
                    .buffer(index_buffer)
                    .build(),

                vk::BindBufferMemoryInfo::builder()
                    .memory(staging_memory)
                    .memory_offset(0)
                    .buffer(staging_buffer)
                    .build()
            ];

            device
                .bind_buffer_memory2(&bind_infos)
                .expect("Failed to associate memory with buffer");
        };

        let staging_mapped_ptr = unsafe { device
            .map_memory(
                staging_memory,
                0, 
                vertex_buffer_size + index_buffer_size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap() as *mut u8
        };

        let block_levels = 8;
        let vertex_allocator = unsafe { crate::allocator::Allocator::new(
            staging_mapped_ptr,
            vertex_buffer_size as usize,
            block_levels,
        ) };

        let index_allocator = unsafe { crate::allocator::Allocator::new(
            (staging_mapped_ptr as vk::DeviceSize + vertex_buffer_size) as *mut u8,
            vertex_buffer_size as usize,
            block_levels,
        ) };

        let max_id_count = 100;
        let id_to_geometry = vec![Default::default(); max_id_count];
        let id_to_geometry_dealloc = vec![Default::default(); max_id_count];
        let available_ids = (0..max_id_count as GeometryId).collect::<Vec<_>>();

        Self {
            device,
            due_vertex_buffer_copies: vec![], // aaaaaaaaaaa
            due_index_buffer_copies: vec![], // TODO: optimize

            id_to_geometry,
            id_to_geometry_dealloc,
            id_exists: utils::new_bitmask_vec(max_id_count, false),
            available_ids,
            geometry_count: 0,

            vertex_buffer,
            vertex_allocator,

            index_buffer,
            index_allocator,

            memory,

            staging_buffer,
            staging_memory,
        }
    }

    pub unsafe fn cmd_bind_resources(&self, command_buffer: vk::CommandBuffer) {
        self.device.cmd_bind_vertex_buffers(
            command_buffer, 
            0, 
            &[self.vertex_buffer], 
            &[0]
        );
        self.device.cmd_bind_index_buffer(
            command_buffer, 
            self.index_buffer, 
            0,
            VK_INDEX_TYPE,
        );
    }

    pub fn create_geometry(
        &mut self, 
        vertices: &[Vertex], 
        indices: &[Index]
    ) -> GeometryId {
        let id = self.available_ids.pop().unwrap();
        self.geometry_count += 1;

        let vertices_size = vertices.len() * size_of::<Vertex>();
        let indices_size = indices.len() * size_of::<Index>();

        let (vertex_ptr, vertex_block_level, vertex_free_tree_index) = unsafe { self.vertex_allocator.allocate(vertices_size) };
        let (index_ptr, index_block_level, index_free_tree_index) = unsafe { self.index_allocator.allocate(indices_size) };

        unsafe {
            (vertex_ptr as *mut Vertex).copy_from(vertices.as_ptr(), vertices.len());
            (index_ptr as *mut Index).copy_from(indices.as_ptr(), indices.len());
        }

        assert!(!utils::get_bit(&self.id_exists, id as usize));
        let vertex_offset = vertex_ptr as vk::DeviceSize - self.vertex_allocator.heap_start as vk::DeviceSize;
        let index_offset = index_ptr as vk::DeviceSize - self.index_allocator.heap_start as vk::DeviceSize;

        self.id_to_geometry[id as usize] = Geometry {
            vertex_offset: vertex_offset as i32,
            first_index: index_offset as u32 / size_of::<u32>() as u32,
            index_count: indices.len() as u32,
        };
        self.id_to_geometry_dealloc[id as usize] = GeometryDealloc {
            vertex_block_level,
            vertex_free_tree_index,
            index_block_level,
            index_free_tree_index,
        };

        self.due_vertex_buffer_copies.push(vk::BufferCopy{
            src_offset: vertex_offset + 0,
            dst_offset: vertex_offset,
            size: vertices_size as vk::DeviceSize,
        });
        self.due_index_buffer_copies.push(vk::BufferCopy{
            src_offset: index_offset + self.vertex_allocator.heap_size as vk::DeviceSize,
            dst_offset: index_offset,
            size: indices_size as vk::DeviceSize,
        });

        id
    }

    pub fn cmd_upload_geometries(&mut self, command_buffer: vk::CommandBuffer) {
        unsafe {
            self.device.cmd_copy_buffer(
                command_buffer, 
                self.staging_buffer, 
                self.vertex_buffer, 
                &self.due_vertex_buffer_copies,
            );

            self.device.cmd_copy_buffer(
                command_buffer, 
                self.staging_buffer, 
                self.index_buffer, 
                &self.due_index_buffer_copies,
            );
        }
        self.due_vertex_buffer_copies.clear();
        self.due_index_buffer_copies.clear();
    }

    pub fn destroy_geometry(&mut self, id: GeometryId) {
        self.geometry_count -= 1;

        unsafe {
            self.vertex_allocator.deallocate(
                (self.id_to_geometry[id as usize].vertex_offset + self.vertex_allocator.heap_start as i32) as *mut u8, 
                self.id_to_geometry_dealloc[id as usize].vertex_block_level,
                self.id_to_geometry_dealloc[id as usize].vertex_free_tree_index,
            );
            self.index_allocator.deallocate(
                (self.id_to_geometry[id as usize].first_index * size_of::<Index>() as u32 + self.index_allocator.heap_start as u32) as *mut u8, 
                self.id_to_geometry_dealloc[id as usize].index_block_level,
                self.id_to_geometry_dealloc[id as usize].index_free_tree_index,
            );
        }

        self.id_to_geometry[id as usize] = Default::default();
        self.available_ids.push(id);
    }

    pub fn cmd_draw_geometry(&self, command_buffer: vk::CommandBuffer, id: GeometryId) {
        assert!(utils::get_bit(&self.id_exists, id as usize));
        let geometry = &self.id_to_geometry[id as usize];

        unsafe { self.device.cmd_draw_indexed(
            command_buffer, 
            geometry.index_count, 
            1, // optimize using instancing
            geometry.first_index, 
            geometry.vertex_offset,
            0, 
        ) };
    }

    /// destroys all resources owned by this geometry system
    pub unsafe fn destroy_resources(&mut self) {
        unsafe {
            self.device.unmap_memory(self.staging_memory);

            self.device.destroy_buffer(self.vertex_buffer, None);
            self.device.destroy_buffer(self.index_buffer, None);
            self.device.destroy_buffer(self.staging_buffer, None);

            self.device.free_memory(self.memory, None);
            self.device.free_memory(self.staging_memory, None);
        }

    }
}
