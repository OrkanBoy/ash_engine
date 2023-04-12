use ash::vk;

use std::{ffi::c_void, rc::Rc};

pub struct Buffer {
    device: Rc<ash::Device>,
    pub handle: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub size: vk::DeviceSize,
    mapped_ptr: Option<*mut c_void>,
}

impl Buffer {
    pub fn new(
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        mem_props: vk::MemoryPropertyFlags,
        device: Rc<ash::Device>,
        device_mem_props: &vk::PhysicalDeviceMemoryProperties,
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

        Self {
            device,
            handle,
            memory,
            size,
            mapped_ptr: None,
        }
    }

    pub fn mapped_ptr(&mut self) -> *mut c_void {
        if let Some(ptr) = self.mapped_ptr {
            ptr
        } else {
            let ptr = unsafe { self.device.map_memory(self.memory, 0, self.size, vk::MemoryMapFlags::empty()) }.expect("Failed to obtain CPU pointer to device memory");
            self.mapped_ptr = Some(ptr);
            ptr
        }
    }

    pub fn copy_from_buffer(
        &mut self, 
        src: &Self,
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

    pub fn new_local_with_data<A, T: Copy>(
        data: &[T],
        usage: vk::BufferUsageFlags,
        transfer_queue: vk::Queue,
        command_pool: vk::CommandPool,
        device: Rc<ash::Device>,
        device_mem_props: &vk::PhysicalDeviceMemoryProperties,
    ) -> Buffer {
        let size = (std::mem::size_of::<T>() * data.len()) as vk::DeviceSize;
        let mut staging_buffer = Self::new(
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            Rc::clone(&device),
            device_mem_props,
        );

        unsafe {
            let memory_ptr = staging_buffer.mapped_ptr();

            let mut align = ash::util::Align::new(memory_ptr, std::mem::align_of::<u32>() as _, size);
            align.copy_from_slice(data);
        }

        let mut buffer = Buffer::new(
            size,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL, 
            Rc::clone(&device), 
            device_mem_props,
        );

        buffer.copy_from_buffer(
            &staging_buffer,
            size,
            transfer_queue,
            command_pool,
        );

        staging_buffer.destroy();

        buffer
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

    pub fn destroy(&mut self) {
        unsafe {
            if self.mapped_ptr.is_some() { self.device.unmap_memory(self.memory); }
            self.device.free_memory(self.memory, None);
            self.device.destroy_buffer(self.handle, None);
        }
    }
}