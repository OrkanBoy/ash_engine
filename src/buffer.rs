use ash::vk;

use std::{ffi::c_void, rc::Rc};

pub struct Buffer<'a> {
    device: Rc<&'a ash::Device>,
    handle: vk::Buffer,
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
    mapped_ptr: *mut c_void,
}

impl<'a> Buffer<'a> {
    fn new(
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        mem_props: vk::MemoryPropertyFlags,
        device: &'a ash::Device,
        device_mem_props: vk::PhysicalDeviceMemoryProperties,
    ) -> Self {
        let handle = {
            let info = vk::BufferCreateInfo::builder()
                .size(size)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            unsafe { device.create_buffer(&info, None) }.expect("Failed to create buffer handle")
        };

        
        let mem_requirements = unsafe { device.get_buffer_memory_requirements(handle) };
        let memory = {
            let mem_type_index = Self::find_mem_type_index(
                mem_requirements.memory_type_bits, 
                mem_props, 
                &device_mem_props
            );
            let alloc_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(mem_requirements.size)
                .memory_type_index(mem_type_index);

            unsafe { device.allocate_memory(&alloc_info, None) }.expect("Failed to allocate device memory")
        };

        unsafe { device.bind_buffer_memory(handle, memory, 0).expect("Failed to associate memory with buffer"); }

        let mapped_ptr = unsafe { device.map_memory(memory, 0, size, vk::MemoryMapFlags::empty()) }.expect("Failed to obtain CPU pointer to device memory");

        Self {
            device: Rc::from(device),
            handle,
            memory,
            size,
            mapped_ptr,
        }
    }

    fn copy_from(
        &mut self, 
        src: Self,
        size: vk::DeviceSize,
        transfer_queue: vk::Queue,
        command_pool: vk::CommandPool,
    ) {
        let command_buffers = {
            let info = vk::CommandBufferAllocateInfo::builder()
                .command_buffer_count(1)
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY);

            unsafe {self.device.allocate_command_buffers(&info).unwrap()}
        };
        let command_buffer = command_buffers[0];

        //begin recording
        {
            let begin_info = vk::CommandBufferBeginInfo::builder()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            unsafe { self.device.begin_command_buffer(command_buffer, &begin_info).unwrap(); }
        }

        //copy
        {
            let regions = [
                vk::BufferCopy {
                    src_offset: 0,
                    dst_offset: 0,
                    size,
                }
            ];
            unsafe { self.device.cmd_copy_buffer(command_buffer, src.handle, self.handle, &regions); }
        }


        //end recording
        unsafe {self.device.end_command_buffer(command_buffer).unwrap();}

        //submit
        {
            let submit_infos = [
                vk::SubmitInfo::builder()
                    .command_buffers(&command_buffers) 
                    .build()
            ];
            unsafe {
                self.device.queue_submit(transfer_queue, &submit_infos, vk::Fence::null()).unwrap();
                self.device.queue_wait_idle(transfer_queue).unwrap();
            }
        }

        unsafe { self.device.free_command_buffers(command_pool, &command_buffers); }
    }

    fn new_with_data<T>(
        data: &[T],
        command_pool: vk::CommandPool,
    ) {

    }

    fn new_with_data_cmd<T>(
        data: &[T],
        command_buffer: vk::CommandBuffer,
        command_pool: vk::CommandPool,
    ) {

    }

    fn find_mem_type_index(
        supported_types_mask: u32,
        required_props: vk::MemoryPropertyFlags,
        props: &vk::PhysicalDeviceMemoryProperties,
        ) -> u32 {
        for i in 0..props.memory_type_count {
            if supported_types_mask & (1 << i) != 0 
                && props.memory_types[i as usize].property_flags.contains(required_props) {
                    return i;
            }
        }
        panic!("Could not find suitable memory type");
    }
}

impl<'a> Drop for Buffer<'a> {
    fn drop(&mut self) {
        unsafe {
            self.device.unmap_memory(self.memory);
            self.device.free_memory(self.memory, None);
            self.device.destroy_buffer(self.handle, None);
        }
    }
}