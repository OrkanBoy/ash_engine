use ash::vk;
use std::{
    marker::PhantomData,
    rc::Rc,
};

pub struct Buffer<T: Copy> {
    device: Rc<ash::Device>,
    pub handle: vk::Buffer,
    pub memory: vk::DeviceMemory,
    size: vk::DeviceSize,
    size_of_t: vk::DeviceSize,
    phantom: PhantomData<T>,
}

impl<T: Copy> Buffer<T> {
    pub fn new(
        count: usize,
        usage: vk::BufferUsageFlags,
        mem_props: vk::MemoryPropertyFlags,
        device: Rc<ash::Device>,
        device_mem_props: &vk::PhysicalDeviceMemoryProperties,
    ) -> Self {
        let size_of_t = std::mem::size_of::<T>() as vk::DeviceSize;
        let size = size_of_t * count as vk::DeviceSize;
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
            size_of_t,
            phantom: PhantomData,
        }
    }

    pub fn copy_from_slice<A>(
        &mut self, 
        slice: &[T], 
        start_index: usize
    ) {
        unsafe {
            let mapped_ptr = self.device.map_memory(
                self.memory, 
                self.size_of_t * start_index as vk::DeviceSize, 
                self.size_of_t * slice.len() as vk::DeviceSize, 
                vk::MemoryMapFlags::empty()
            ).expect("Failed to obtain CPU pointer to GPU memory");

            let mut align = ash::util::Align::new(mapped_ptr, std::mem::size_of::<A>() as vk::DeviceSize, self.size);
            align.copy_from_slice(slice);

            self.device.unmap_memory(self.memory);
        }
    }

    pub fn copy_from_buffer(
        &mut self, 
        src: &Self,
        start_index: usize,
        count: usize,
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
                    dst_offset: start_index as vk::DeviceSize * self.size_of_t,
                    size: self.size_of_t * count as vk::DeviceSize,
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

    pub fn stage_and_copy_from_slice<A>(
        &mut self,
        data: &[T],
        start_index: usize,
        transfer_queue: vk::Queue,
        command_pool: vk::CommandPool,
        device_mem_props: &vk::PhysicalDeviceMemoryProperties,
    ) {
        let mut staging_buffer = Self::new(
            data.len(),
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            self.device.clone(),
            device_mem_props,
        );

        staging_buffer.copy_from_slice::<A>(data, 0);

        self.copy_from_buffer(
            &staging_buffer,
            start_index,
            data.len(),
            transfer_queue,
            command_pool,
        );

        staging_buffer.destroy();
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
            self.device.free_memory(self.memory, None);
            self.device.destroy_buffer(self.handle, None);
        }
    }
}