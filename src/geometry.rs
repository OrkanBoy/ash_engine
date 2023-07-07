pub struct Vertex {
    pub x: f32, pub y: f32, pub z: f32,

    // r: f32, g: f32, b: f32,
    
    // u: f32, v: f32,
}

use std::rc::Rc;
use core::mem::{size_of, align_of};

use ash::vk;

use crate::memory::{Allocator, buddy, self};

type GeometryId = usize;
type Index = u32;

// TODO: configurable
const VK_INDEX_TYPE: vk::IndexType = vk::IndexType::UINT32;

/// referred by a geometry id from user and used internally for binding that geometry
#[derive(Clone)]
struct Geometry {
    vertex_offset:  i32,
    first_index:    u32,
    index_count:    u32,
}

/// Holds static geometry. 
/// User provides vertex and index data and system loads data onto device local memory
/// System also returns back geometry id which refers to the loaded geometry
pub struct GeometrySystem {
    device:                     Rc<ash::Device>,
    due_vertex_buffer_copies:   Vec<vk::BufferCopy>,
    due_index_buffer_copies:    Vec<vk::BufferCopy>,
    
    id_to_geometry:             Vec<Option<Geometry>>,
    available_ids:              Vec<GeometryId>,
    geometry_count:             usize,

    vertex_buffer:              vk::Buffer,
    index_buffer:               vk::Buffer,
    memory:                     vk::DeviceMemory,
    
    staging_buffer:             vk::Buffer,
    staging_memory:             vk::DeviceMemory,

    vertex_allocator:           memory::buddy::BuddyAllocator,
    index_allocator:            memory::buddy::BuddyAllocator,
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
        let vertex_allocator = unsafe { crate::memory::buddy::BuddyAllocator::new(
            staging_mapped_ptr,
            vertex_buffer_size as usize,
            block_levels,
        ) };

        let index_allocator = unsafe { crate::memory::buddy::BuddyAllocator::new(
            (staging_mapped_ptr as vk::DeviceSize + vertex_buffer_size) as *mut u8,
            vertex_buffer_size as usize,
            block_levels,
        ) };

        let id_to_geometry = vec![None; 100];
        let available_ids = (0..100).collect::<Vec<_>>();

        Self {
            device,
            due_vertex_buffer_copies: vec![], // aaaaaaaaaaa
            due_index_buffer_copies: vec![], // TODO: optimize

            id_to_geometry,
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

        let (vertex_ptr, _) = unsafe { self.vertex_allocator.alloc(
            vertices_size,
            0 // ignored for buddy allocator anyways
        )};
        let (index_ptr, _) = unsafe { self.index_allocator.alloc(
            indices_size, 
            0 // ignored for buddy allocator anyways
        )};

        unsafe {
            (vertex_ptr as *mut Vertex).copy_from(vertices.as_ptr(), vertices.len());
            (index_ptr as *mut Index).copy_from(indices.as_ptr(), indices.len());
        }

        assert!(self.id_to_geometry[id].is_none());
        let vertex_offset = vertex_ptr as vk::DeviceSize - self.vertex_allocator.heap_start as vk::DeviceSize;
        let index_offset = index_ptr as vk::DeviceSize - self.index_allocator.heap_start as vk::DeviceSize;

        self.id_to_geometry[id] = Some(Geometry {
            vertex_offset: vertex_offset as i32,
            first_index: index_offset as u32 / size_of::<u32>() as u32,
            index_count: indices.len() as u32,
        });

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
        assert!(self.due_vertex_buffer_copies.len() != 0 && self.due_index_buffer_copies.len() != 0);
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
        let geometry = self.id_to_geometry[id].as_ref().unwrap();
        self.geometry_count -= 1;

        unsafe {
            self.vertex_allocator.dealloc(
                (geometry.vertex_offset + self.vertex_allocator.heap_start as i32) as *mut u8, 
            );
            self.index_allocator.dealloc(
                (geometry.first_index * size_of::<Index>() as u32 + self.index_allocator.heap_start as u32) as *mut u8, 
            );
        }

        self.id_to_geometry[id] = None;
        self.available_ids.push(id);
    }

    pub fn cmd_draw_geometry(&self, command_buffer: vk::CommandBuffer, id: GeometryId) {
        let geometry = self.id_to_geometry[id].as_ref().unwrap();
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
