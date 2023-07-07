pub mod debug;
pub mod buffer;
pub mod device;
pub mod swapchain;
pub mod pipeline;
pub mod descriptor;
pub mod texture;
pub mod image;
pub mod render_pass;

use buffer::Buffer;
use crate::camera::Camera;

use raw_window_handle::{
    HasRawDisplayHandle, 
    HasRawWindowHandle,
};

use std::{
    ffi::CString, 
    rc::Rc, 
    time, mem::size_of, 
};

use ash::{
    vk::{
        self, 
    },
    extensions::{
        khr::{
            Surface, 
            Win32Surface, 
            Swapchain
        }, 
        ext::DebugUtils
    }, 
};

use self::descriptor::PerFrameUBO;

pub const START_WINDOW_WIDTH: u32 = 1280;
pub const START_WINDOW_HEIGHT: u32 = 720;

pub const MAX_FRAMES_IN_FLIGHT: usize = 1;

pub struct VkApp {
    pub camera: Camera,
    pub input_state: crate::input::InputState,
    pub in_game: bool,
    pub start_instant: time::Instant,

    entry: ash::Entry,
    instance: ash::Instance,
    shader_compiler: shaderc::Compiler,

    pub window: winit::window::Window,
    surface: Surface,
    surface_khr: vk::SurfaceKHR,

    debug_utils: DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT, 

    physical_device: vk::PhysicalDevice,
    device: Rc<ash::Device>,

    graphics_command_pool: vk::CommandPool,
    descriptor_pool: vk::DescriptorPool,
    transient_command_pool: vk::CommandPool,

    physical_device_memory_properties: vk::PhysicalDeviceMemoryProperties,

    graphics_queue: vk::Queue,
    transfer_queue: vk::Queue,
    present_queue: vk::Queue,

    graphics_family_index: u32,
    present_family_index: u32,
    transfer_family_index: u32,

    swapchain: Swapchain, 
    swapchain_khr: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    swapchain_image_format: vk::Format,
    pub swapchain_extent: vk::Extent2D,
    swapchain_framebuffers: Vec<vk::Framebuffer>,
    swapchain_depth_format: vk::Format,
    swapchain_depth_image: vk::Image,
    swapchain_depth_image_memory: vk::DeviceMemory,
    swapchain_depth_image_view: vk::ImageView,

    render_pass: vk::RenderPass,

    // Improve uniform buffer object and descriptor set system
    per_frame_ubo_set_layout: vk::DescriptorSetLayout,
    per_frame_ubo_set: vk::DescriptorSet,

    // proper texture system
    // and resource acquisition
    textures_set_layout: vk::DescriptorSetLayout,
    textures_sets: Vec<vk::DescriptorSet>,
    textures: Vec<texture::Texture>,

    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,

    graphics_command_buffers: Vec<vk::CommandBuffer>,

    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,

    vertex_buffer: Buffer, // allocator
    index_buffer: Buffer, // allocator

    per_frame_uniform_buffer: Buffer,

    current_frame: usize,
}

impl VkApp {
    pub fn new(window: winit::window::Window) -> Self {
        log::debug!("Creating app...");

        let entry = ash::Entry::linked();
        let instance = Self::new_instance(&entry);

        let surface = Surface::new(&entry, &instance);
        let surface_khr = unsafe { ash_window::create_surface(
            &entry,
            &instance,
            window.raw_display_handle(), 
            window.raw_window_handle(), 
            None,
        ).expect("Failed to acquire vulkan window handle(surface)") };
        
        let debug_utils = DebugUtils::new(&entry, &instance);
        let debug_messenger = debug::new_messenger(&debug_utils);

        let (physical_device,

            graphics_family_index,
            present_family_index,
            transfer_family_index,
        ) = device::get_physical_device_and_queue_family_indices(
            &instance, 
            &surface, 
            surface_khr,
        );

        let (device, 

            graphics_queue, 
            present_queue,
            transfer_queue,
        ) = device::new_logical_device_and_queues(
            &instance,
            physical_device,
            graphics_family_index,
            present_family_index,
            transfer_family_index
        );

        let graphics_command_pool = Self::new_command_pool(
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            graphics_family_index,
            &device,
        );
        let graphics_command_buffers = Self::new_command_buffers(
            &device, 
            graphics_command_pool,
            MAX_FRAMES_IN_FLIGHT,
        );
        let transient_command_pool = Self::new_command_pool(
            vk::CommandPoolCreateFlags::TRANSIENT,
            graphics_family_index,
            &device,
        );

        let (swapchain, 
            swapchain_khr, 
            swapchain_images,
            swapchain_image_views,
            swapchain_image_format, 
            swapchain_extent
        ) = swapchain::new_swapchain_and_images(
            &instance, 
            physical_device,
            &device,
            &surface, 
            surface_khr, 
            vk::Extent2D{
                width: START_WINDOW_WIDTH, 
                height: START_WINDOW_HEIGHT
            },
            graphics_family_index,
            present_family_index,
        );

        let swapchain_depth_format = device::find_depth_format(&instance, physical_device);
        log::info!("Picked depth format {:?}", swapchain_depth_format);
        let render_pass = render_pass::new_render_pass(
            &device,
            swapchain_image_format,
            swapchain_depth_format,
        );

        let (
            per_frame_ubo_set_layout, 
            textures_set_layout,
        ) = descriptor::new_descriptor_set_layouts(&device, 1);
        
        
        use pipeline::Attribute;
        let shader_compiler = shaderc::Compiler::new().unwrap();
        let (pipeline, pipeline_layout) = pipeline::new_pipeline_and_layout(
            &device, 
            &shader_compiler,
            render_pass,
            per_frame_ubo_set_layout,
            textures_set_layout,
            "shaders/foo.vert",
            "shaders/foo.frag",
            &[
                Attribute::F32x3,
                Attribute::F32x3,
                Attribute::F32x2,
            ],
            &[
                Attribute::F32x4x3,
            ],
        );

        let physical_device_memory_properties = unsafe { 
            instance.get_physical_device_memory_properties(physical_device) 
        };

        let vertex_buffer = Buffer::new(
            4 * 100,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device.clone(),
            &physical_device_memory_properties,
        );
        let index_buffer = Buffer::new(
            2 * 200,
            vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device.clone(),
            &physical_device_memory_properties,
        );
        let per_frame_uniform_buffer = Buffer::new(
            2 * 200,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
            device.clone(),
            &physical_device_memory_properties,
        );

        let (swapchain_depth_image, swapchain_depth_image_memory, swapchain_depth_image_view) = Self::new_depth_resources(
            &device,
            &physical_device_memory_properties,
            transient_command_pool,
            graphics_queue,
            graphics_family_index,
            swapchain_depth_format,
            swapchain_extent,
        );

        let swapchain_framebuffers = swapchain::new_swapchain_framebuffers(
            &device, 
            &swapchain_image_views,
            swapchain_depth_image_view,
            render_pass, 
            swapchain_extent,
        );
        
        let descriptor_pool = descriptor::new_descriptor_pool(&device);
        let per_frame_ubo_set = descriptor::new_per_frame_ubo_set(
            &device, 
            descriptor_pool, 
            per_frame_ubo_set_layout, 
            &per_frame_uniform_buffer,
        );

        let mut image_available_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut render_finished_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut in_flight_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
    
        let semaphore_info = &vk::SemaphoreCreateInfo::builder();
        let fence_info = &vk::FenceCreateInfo::builder()
            .flags(vk::FenceCreateFlags::SIGNALED);

        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            image_available_semaphores.push(
                unsafe { device.create_semaphore(semaphore_info, None).unwrap() }
            );
            render_finished_semaphores.push(
                unsafe { device.create_semaphore(semaphore_info, None).unwrap() }
            );
            in_flight_fences.push(
                unsafe { device.create_fence(fence_info, None).unwrap() }
            );
        }

        let camera = Camera {
            translation: crate::math::Vector { x: 0.0, y: 0.0, z: -4.0 },
            z_x_angle: 0.0,
            y_xz_angle: 0.0,
            near_z: 1.0,
            far_z: 100.0,
            aspect_ratio: START_WINDOW_WIDTH as f32 / START_WINDOW_HEIGHT as f32,
            translation_speed: 3.0,
            rotation_speed: 0.2,
        };

        let input_state = crate::input::InputState::new();

        Self {
            camera,
            input_state, 
            in_game: false,

            start_instant: time::Instant::now(),
            entry,
            instance,
            shader_compiler,

            window,
            surface,
            surface_khr,

            debug_utils,
            debug_messenger,

            physical_device,
            device,

            graphics_command_pool,
            transient_command_pool,
            descriptor_pool,

            physical_device_memory_properties,

            graphics_queue,
            transfer_queue,
            present_queue,

            graphics_family_index, 
            transfer_family_index,
            present_family_index,

            swapchain,
            swapchain_khr, 
            swapchain_images,
            swapchain_image_views,
            swapchain_image_format,
            swapchain_extent,
            swapchain_framebuffers,
            swapchain_depth_format,
            swapchain_depth_image,
            swapchain_depth_image_memory,
            swapchain_depth_image_view,

            render_pass,

            per_frame_ubo_set_layout,
            per_frame_ubo_set,
            per_frame_uniform_buffer,

            textures_set_layout,
            textures: vec![],
            textures_sets: vec![],

            pipeline_layout,
            pipeline,
   
            graphics_command_buffers,

            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,

            vertex_buffer,
            index_buffer,

            current_frame: 0,
        }
    }

    pub fn execute_transient_commands<F: FnOnce(vk::CommandBuffer)>(
        device: &ash::Device,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
        executor: F,
    ) {
        let alloc_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(1)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_pool(command_pool);

        let command_buffer = unsafe {device.allocate_command_buffers(&alloc_info)}.unwrap()[0];

        let begin_info = vk::CommandBufferBeginInfo::builder();

        unsafe {
            device.begin_command_buffer(command_buffer, &begin_info).unwrap();
        }

        executor(command_buffer);

        unsafe {
            device.end_command_buffer(command_buffer).unwrap();
        }

        let command_buffers = &[command_buffer];

        let submit_info = vk::SubmitInfo::builder()
            .command_buffers(command_buffers)
            .build();

        unsafe {
            device.queue_submit(queue, &[submit_info], vk::Fence::null()).unwrap();
            device.device_wait_idle().unwrap();

            device.free_command_buffers(command_pool, command_buffers);
        }
    }

    /// Create the depth buffer resources (image, memory and view).
    /// 
    /// This function also transitions the image to be ready to be used
    /// as a depth/stencil attachement.
    fn new_depth_resources(
        device: &ash::Device,
        physical_device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
        transition_command_pool: vk::CommandPool,
        transition_queue: vk::Queue,
        transition_family_index: u32,
        format: vk::Format,
        swapchain_extent: vk::Extent2D,
    ) -> (vk::Image, vk::DeviceMemory, vk::ImageView) {
        let (image, memory) = image::new_image_and_memory(
            device,
            physical_device_memory_properties,
            swapchain_extent.width,
            swapchain_extent.height,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            format,
            vk::ImageTiling::OPTIMAL,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );

        Self::execute_transient_commands(
            device, 
            transition_command_pool, 
            transition_queue, 
            |transfer_command_buffer|
                image::cmd_transition_image_layout(
                    device,
                    image,
                    transfer_command_buffer,
                    transition_family_index,
                    format,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                )
        );

        let view = image::new_image_view(
            device, 
            image, 
            format, 
            vk::ImageAspectFlags::DEPTH
        );

        (image, memory, view)
    }


    // TODO: swapchain abstraction
    pub fn renew_swapchain(&mut self) {
        self.cleanup_swapchain();

        (
            self.swapchain, 
            self.swapchain_khr, 
            self.swapchain_images, 
            self.swapchain_image_views,
            self.swapchain_image_format, 
            self.swapchain_extent
        ) = swapchain::new_swapchain_and_images(
            &self.instance, 
            self.physical_device, 
            &self.device, 
            &self.surface, 
            self.surface_khr, 
            self.swapchain_extent,
            self.graphics_family_index,
            self.present_family_index,
        );

        (
            self.swapchain_depth_image,
            self.swapchain_depth_image_memory,
            self.swapchain_depth_image_view,
        ) = Self::new_depth_resources(
            &self.device,
            &self.physical_device_memory_properties,
            self.graphics_command_pool,
            self.graphics_queue,
            self.graphics_family_index,
            self.swapchain_depth_format,
            self.swapchain_extent,
        );

        self.swapchain_framebuffers = swapchain::new_swapchain_framebuffers(
            &self.device, 
            &self.swapchain_image_views,
            self.swapchain_depth_image_view,
            self.render_pass, 
            self.swapchain_extent
        );
    }
    
    fn cleanup_swapchain(&mut self) {
        unsafe {
            //TODO:  = no good
            self.device.device_wait_idle().unwrap();

            self.device.destroy_image_view(self.swapchain_depth_image_view, None);
            self.device.destroy_image(self.swapchain_depth_image, None);
            self.device.free_memory(self.swapchain_depth_image_memory, None);

            for i in 0..self.swapchain_images.len() {
                self.device.destroy_framebuffer(self.swapchain_framebuffers[i], None);
                self.device.destroy_image_view(self.swapchain_image_views[i], None);
            }

            self.swapchain.destroy_swapchain(self.swapchain_khr, None);
        }
    }

    fn new_command_buffers(
        device: &ash::Device, 
        pool: vk::CommandPool,
        count: usize,
    ) -> Vec<vk::CommandBuffer> {
        //allocate command command_buffers
        let alloc_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(count as u32);

        unsafe { device.allocate_command_buffers(&alloc_info).unwrap() }
    }

    fn new_command_pool(
        create_flags: vk::CommandPoolCreateFlags,
        queue_family_index: u32,
        device: &ash::Device,
    ) -> vk::CommandPool {
        let info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_family_index)
            .flags(create_flags);

        unsafe { device.create_command_pool(&info, None).expect("Failed to create command pool") }
    }

    fn new_instance(entry: &ash::Entry) -> ash::Instance {
        let app_name = CString::new("Vulkan Application").unwrap();
        let engine_name = CString::new("No Engine").unwrap();

        let app_info = vk::ApplicationInfo::builder()
            .application_name(&app_name)
            .engine_name(&engine_name)
            .application_version(vk::make_api_version(0, 0, 0, 1))
            .engine_version(vk::make_api_version(0, 0, 0, 1))
            .api_version(vk::make_api_version(0, 1, 0, 0));

        let extension_name_ptrs = [
            ash::extensions::khr::Surface::name().as_ptr(), 
            Win32Surface::name().as_ptr(),
            #[cfg(debug_assertions)] 
            DebugUtils::name().as_ptr()
        ];
        let (_, layer_name_ptrs) = &debug::get_layer_names_and_ptrs();

        let mut info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_name_ptrs);
            
        #[cfg(debug_assertions)] {
            debug::check_validation_layer_support(entry);
            info = info.enabled_layer_names(&layer_name_ptrs);
        }

        unsafe { entry.create_instance(&info, None).unwrap() }
    }

    fn update_uniform_buffer(&mut self) {
        let ubo = descriptor::PerFrameUBO {
            proj_view: self.camera.calc_proj_view()
        };

        self.per_frame_uniform_buffer.copy_from_slice(
            &[ubo], 
            (self.current_frame * size_of::<PerFrameUBO>()) as vk::DeviceSize
        );
    }

    fn record_graphics_command_buffer(
        &mut self, 
        graphics_command_buffer: vk::CommandBuffer,
        image_index: usize,
    ) {
        let begin_info = vk::CommandBufferBeginInfo::default();
        
        let render_area = vk::Rect2D {
            offset: vk::Offset2D{
                x: 0, y: 0,
            },
            extent: self.swapchain_extent,
        };

        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.2, 1.0],
                }
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                }
            },
        ];
        
        let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
            .render_pass(self.render_pass)
            .framebuffer(self.swapchain_framebuffers[image_index])
            .render_area(render_area)
            .clear_values(&clear_values);
        
        let viewport = vk::Viewport {
            x: 0.0, 
            y: 0.0,
            width: self.swapchain_extent.width as f32, 
            height: self.swapchain_extent.height as f32,
            min_depth: 0.0, 
            max_depth: 1.0, 
        };
        let scissor = vk::Rect2D {
            offset: vk::Offset2D {
                x: 0,
                y: 0,
            },
            extent: self.swapchain_extent,
        };

        unsafe {
            self.device.begin_command_buffer(
                graphics_command_buffer, 
                &begin_info
            ).expect("Failed to begin recording command buffer");

            self.device.cmd_begin_render_pass(
                graphics_command_buffer, 
                &render_pass_begin_info, 
                vk::SubpassContents::INLINE
            );

            self.device.cmd_set_viewport(
                graphics_command_buffer, 
                0, 
                &[viewport]
            );
            self.device.cmd_set_scissor(
                graphics_command_buffer, 
                0, 
                &[scissor]
            );

            self.device.cmd_bind_pipeline(
                graphics_command_buffer, 
                vk::PipelineBindPoint::GRAPHICS, 
                self.pipeline
            );

            self.device.cmd_end_render_pass(graphics_command_buffer);

            self.device.end_command_buffer(graphics_command_buffer).expect("Could not end recording command buffer");
        }
        
    }

    fn wait_for_and_reset_fences(&mut self, fences: &[vk::Fence]) {
        unsafe {
            self.device.wait_for_fences(fences, true, u64::MAX).unwrap();
            self.device.reset_fences(fences).unwrap();
        }
    }

    fn reset_command_buffer(&mut self, command_buffer: vk::CommandBuffer) {
        unsafe {
            self.device.reset_command_buffer(
                command_buffer, 
                vk::CommandBufferResetFlags::empty()
            ).expect("Failed to reset command buffer contents"); 
        }
    }

    /// returns wether swapchain is dirty
    pub fn draw_frame(&mut self) -> bool {
        log::trace!("Drawing frame...");

        let image_available_semaphore = self.image_available_semaphores[self.current_frame];
        let render_finished_semaphore = self.render_finished_semaphores[self.current_frame];
        let in_flight_fence = self.in_flight_fences[self.current_frame];

        let graphics_command_buffer = self.graphics_command_buffers[self.current_frame];

        self.wait_for_and_reset_fences(&[in_flight_fence]);

        let image_index = unsafe {
            match self.swapchain.acquire_next_image(
                self.swapchain_khr, 
                u64::MAX, 
                image_available_semaphore, 
                vk::Fence::null(),
            ) {
                Ok((image_index, _)) => image_index,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return true,
                Err(err) => panic!("Error acquiring image: {}", err),
            }
        };

        self.reset_command_buffer(graphics_command_buffer);

        self.update_uniform_buffer();

        //render
        self.record_graphics_command_buffer(graphics_command_buffer, image_index as usize);
        {
            let render_info = vk::SubmitInfo::builder()
                .command_buffers(&[graphics_command_buffer])
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                .wait_semaphores(&[image_available_semaphore])
                .signal_semaphores(&[render_finished_semaphore])
                .build();
            let render_infos = [render_info];

            unsafe { self.device.queue_submit(self.graphics_queue, &render_infos, in_flight_fence).unwrap(); }
        }

        //present
        {
            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&[render_finished_semaphore])
                .swapchains(&[self.swapchain_khr])
                .image_indices(&[image_index])
                .build();
            unsafe {
                match self.swapchain.queue_present(self.present_queue, &present_info) {
                    Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return true,
                    Err(err) => panic!("Error presenting: {}", err),
                    _ => {},
                }
            }
        }

        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
        false
    }
}

impl Drop for VkApp {
    fn drop(&mut self) {
        log::debug!("Dropping application...");

        self.cleanup_swapchain();

        unsafe {
            self.vertex_buffer.destroy();
            self.index_buffer.destroy();

            self.per_frame_uniform_buffer.destroy();
            self.device.destroy_descriptor_set_layout(self.per_frame_ubo_set_layout, None);

            for texture in self.textures.iter_mut() {
                texture.destroy();
            }
            self.device.destroy_descriptor_set_layout(self.textures_set_layout, None);

            self.device.destroy_descriptor_pool(self.descriptor_pool, None);

            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);

            for frame in 0..MAX_FRAMES_IN_FLIGHT {
                self.device.destroy_semaphore(self.image_available_semaphores[frame], None);
                self.device.destroy_semaphore(self.render_finished_semaphores[frame], None);
                self.device.destroy_fence(self.in_flight_fences[frame], None);
            }

            self.device.destroy_command_pool(self.graphics_command_pool, None);
            self.device.destroy_command_pool(self.transient_command_pool, None);

            self.device.destroy_render_pass(self.render_pass, None);

            self.device.destroy_device(None);

            self.surface.destroy_surface(self.surface_khr, None);

            #[cfg(debug_assertions)]
            self.debug_utils.destroy_debug_utils_messenger(self.debug_messenger, None);

            self.instance.destroy_instance(None);
        } 
    }
}