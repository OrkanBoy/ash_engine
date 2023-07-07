use ash::vk;
use std::{rc::Rc, ffi::c_void, mem::size_of};

// TODO: separate uniform buffer and buffer as data types
// TODO: understand vulkan memory alignment
pub struct Buffer {
    device: Rc<ash::Device>,

    pub handle: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub size: vk::DeviceSize,
    alignment: vk::DeviceSize,
    alignment_mask: vk::DeviceSize,
}

pub struct BufferSlice;

impl Buffer {
    pub fn new(
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        props: vk::MemoryPropertyFlags,
        device: Rc<ash::Device>,
        physical_device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
    ) -> Self {
        let handle = {
            let info = vk::BufferCreateInfo::builder()
                .size(size)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE); // configurable
            unsafe { device.create_buffer(&info, None) }.expect("Failed to create buffer handle")
        };

        let mem_requirements = unsafe { device.get_buffer_memory_requirements(handle) };

        let memory = {
            let mem_type_index = super::device::find_mem_type_index(
                mem_requirements.memory_type_bits,
                props,
                &physical_device_memory_properties,
            );
            let alloc_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(mem_requirements.size)
                .memory_type_index(mem_type_index);

            unsafe { device.allocate_memory(&alloc_info, None) }
                .expect("Failed to allocate device memory")
        };

        unsafe {
            device
                .bind_buffer_memory(handle, memory, 0)
                .expect("Failed to associate memory with buffer");
        }

        let alignment = mem_requirements.alignment;

        Self {
            device,
            handle,
            memory,
            size,
            alignment,
            alignment_mask: !(alignment - 1),
        }
    }

    pub fn copy_from_slice<T: Copy>(&mut self, slice: &[T], offset: vk::DeviceSize) {
        // assert!(offset & self.alignment_mask == offset);
        // assert!(slice.len() & self.alignment_mask as usize == slice.len());

        unsafe {

            let mapped_ptr = self
                .device
                .map_memory(
                    self.memory,
                    offset,
                    (slice.len() * size_of::<T>()) as vk::DeviceSize,
                    vk::MemoryMapFlags::empty(),
                )
                .expect("Failed to obtain CPU pointer to GPU memory") as *mut T;

            mapped_ptr.copy_from(slice.as_ptr(), slice.len());

            self.device.unmap_memory(self.memory);
        }
    }

    pub fn cmd_copy_from_buffer(
        &mut self,
        src: &Self,
        offset: vk::DeviceSize,
        size: vk::DeviceSize,
        transfer_command_buffer: vk::CommandBuffer,
    ) {
        // assert!(self.alignment == src.alignment);
        // assert!(offset & self.alignment_mask == offset);
        // assert!(size & self.alignment_mask == size);

        let regions = [vk::BufferCopy {
            src_offset: 0,
            dst_offset: offset,
            size,
        }];

        unsafe {
            self.device.cmd_copy_buffer(
                transfer_command_buffer,
                src.handle,
                self.handle,
                &regions,
            );
        }
    }

    // returns the staging buffer to destroy
    pub fn cmd_stage_and_copy_from_slice<T: Copy>(
        &mut self,
        data: &[T],
        offset: vk::DeviceSize,
        transfer_command_buffer: vk::CommandBuffer,
        physical_device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
    ) -> Buffer {
        let size = (size_of::<T>() * data.len()) as vk::DeviceSize;
        let mut staging_buffer = Self::new(
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            self.device.clone(),
            physical_device_memory_properties,
        );

        staging_buffer.copy_from_slice(data, 0);

        self.cmd_copy_from_buffer(
            &staging_buffer,
            offset,
            size,
            transfer_command_buffer,
        );

        staging_buffer
    }

    // caller must ensure only called once
    pub unsafe fn destroy(&mut self) {
        self.device.destroy_buffer(self.handle, None);
        self.device.free_memory(self.memory, None);
    }
}