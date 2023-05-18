pub mod debug;
pub mod data;
pub mod buffer;
pub mod math;
pub mod device;
pub mod swapchain;
pub mod pipeline;
pub mod descriptor;
pub mod instance;
pub mod entity;
pub mod input;

use buffer::Buffer;
use data::VertexData;
use instance::Particle;

use raw_window_handle::{
    HasRawDisplayHandle, 
    HasRawWindowHandle,
};

use std::{
    ffi::CString, 
    rc::Rc, 
    time, 
};

use winit::{
    self, 
    event_loop::EventLoop, 
    window::{WindowBuilder, CursorGrabMode}, 
    dpi::{PhysicalSize, PhysicalPosition}, 
    event::{
        WindowEvent,
        VirtualKeyCode, KeyboardInput, ElementState, DeviceEvent,
    }
};

use ash::{
    vk::{
        self, 
        SampleCountFlags, 
        AttachmentLoadOp, 
        AttachmentStoreOp,
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

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;
const MAX_FRAMES_IN_FLIGHT: usize = 2;
const MAX_VERTICES_COUNT: usize = 100;
const MAX_INDICES_COUNT: usize = 100;
const MAX_PARTICLES_INSTANCE_COUNT: usize = 40;

struct VkApp {
    camera: Camera,
    input_state: input::InputState,
    in_game: bool,

    start_instant: time::Instant,
    entry: ash::Entry,
    instance: ash::Instance,

    window: winit::window::Window,
    surface: Surface,
    surface_khr: vk::SurfaceKHR,

    debug_utils: DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT, 

    physical_device: vk::PhysicalDevice,
    device: Rc<ash::Device>,

    physical_device_mem_props: vk::PhysicalDeviceMemoryProperties,

    graphics_queue: vk::Queue,
    transfer_queue: vk::Queue,
    present_queue: vk::Queue,

    swapchain: Swapchain, 
    swapchain_khr: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    swapchain_image_format: vk::Format,
    swapchain_extent: vk::Extent2D,
    swapchain_framebuffers: Vec<vk::Framebuffer>,

    render_pass: vk::RenderPass,

    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,

    graphics_command_pool: vk::CommandPool,
    graphics_command_buffers: Vec<vk::CommandBuffer>,
    transfer_command_pool: vk::CommandPool,
    transfer_command_buffers: Vec<vk::CommandBuffer>,

    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,

    vertex_buffer: Buffer<data::VertexData>,
    vertex_count: usize,
    vertex_transfer_fence: vk::Fence,

    index_buffer: Buffer<u16>,
    index_count: usize,
    index_transfer_fence: vk::Fence,

    uniform_buffer: Buffer<descriptor::UniformData>,

    particles: Vec<instance::Particle>,
    particle_instance_buffer: Buffer<data::InstanceData>,
    particle_instance_count: usize,
    particle_instances: Vec<instance::Instance>,
    particle_instance_transfer_fences: Vec<vk::Fence>,

    current_frame: usize,
}

impl VkApp {
    fn new(window: winit::window::Window) -> Self {
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
        ).expect("Failed to get acquire vulkan window handle(surface)") };
        
        let debug_utils = DebugUtils::new(&entry, &instance);
        let debug_messenger = debug::new_messenger(&debug_utils);

        let physical_device = device::pick_physical_device(
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
            &surface, 
            surface_khr, 
            physical_device,
        );

        let (
            graphics, 
            transfer,
            _,
        ) = device::find_queue_family_indices(physical_device, &surface, surface_khr, &instance);

        let graphics = graphics.unwrap();
        let transfer = transfer.unwrap();

        let graphics_command_pool = Self::new_command_pool(
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            graphics,
            &device,
        );
        let graphics_command_buffers = Self::new_command_buffers(
            &device, 
            graphics_command_pool, 
        );
        let transfer_command_pool = Self::new_command_pool(
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            transfer,
            &device,
        );
        let transfer_command_buffers = Self::new_command_buffers(
            &device, 
            transfer_command_pool, 
        );

        let (swapchain, 
            swapchain_khr, 
            swapchain_images, 
            swapchain_image_format, 
            swapchain_extent
        ) = swapchain::new_swapchain_and_images(
            &instance, 
            physical_device,
            &device,
            &surface, 
            surface_khr, 
            vk::Extent2D{width: WIDTH, height: HEIGHT}
        );

        let swapchain_image_views = swapchain::new_swapchain_image_views(
            &device, 
            &swapchain_images, 
            swapchain_image_format,
        );

        let render_pass = Self::new_render_pass(
            &device, 
            swapchain_image_format,
        );

        let descriptor_set_layout = descriptor::new_descriptor_set_layout(&device);
        let (pipeline, pipeline_layout) = pipeline::new_pipeline(
            &device, 
            swapchain_extent, 
            render_pass,
            descriptor_set_layout,
        );

        let swapchain_framebuffers = swapchain::new_swapchain_framebuffers(
            &device, 
            &swapchain_image_views, 
            render_pass, 
            swapchain_extent,
        );

        let physical_device_mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        
        let vertex_buffer = Buffer::new(
            MAX_VERTICES_COUNT,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device.clone(),
            &physical_device_mem_props,
        );
        let index_buffer = Buffer::new(
            MAX_INDICES_COUNT,
            vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device.clone(),
            &physical_device_mem_props,
        );
        let uniform_buffer = Buffer::new(
            MAX_FRAMES_IN_FLIGHT,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            device.clone(),
            &physical_device_mem_props,
        );
        let particle_instance_buffer = Buffer::new(
            MAX_PARTICLES_INSTANCE_COUNT * MAX_FRAMES_IN_FLIGHT,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device.clone(),
            &physical_device_mem_props,
        );

        let descriptor_pool = descriptor::new_descriptor_pool(&device, 1);
        let descriptor_set = descriptor::new_descriptor_set(&device, descriptor_pool, descriptor_set_layout, &uniform_buffer);

        let (
            image_available_semaphores, 
            render_finished_semaphores, 
            in_flight_fences,
            vertex_transfer_fence,
            index_transfer_fence,
            particle_instance_transfer_fences,
        ) = Self::new_sync_objects(&device);


        let camera = Camera {
            x: 0.0, y: 0.0, z: -4.0,
            x_z_angle: 0.0,
            xz_y_angle: 0.0,
            near_z: 1.0,
            far_z: 100.0,
            translation_speed: 3.0,
            rotation_speed: 0.2,
        };

        let input_state = input::InputState::new();

        Self {
            camera,
            input_state,
            in_game: false,

            start_instant: time::Instant::now(),
            entry,
            instance,

            window: window,
            surface,
            surface_khr,

            debug_utils,
            debug_messenger,

            physical_device,
            device,

            physical_device_mem_props,

            graphics_queue,
            transfer_queue,
            present_queue,

            swapchain,
            swapchain_khr, 
            swapchain_images,
            swapchain_image_views,
            swapchain_image_format,
            swapchain_extent,
            swapchain_framebuffers,

            render_pass,

            descriptor_set_layout,
            descriptor_pool,
            descriptor_set,
            pipeline_layout,
            pipeline,

            transfer_command_pool,
            graphics_command_pool,
            graphics_command_buffers,
            transfer_command_buffers,

            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,

            vertex_buffer,
            vertex_count: 0,
            vertex_transfer_fence,

            index_buffer,
            index_count: 0,
            index_transfer_fence,

            uniform_buffer,

            particle_instance_buffer, 
            particle_instance_count: 0,
            particle_instance_transfer_fences,
            particles: vec![],
            particle_instances: vec![ //doesn't matter exact value
                instance::Instance {
                    scale: math::Vector::new(1.0, 1.0, 1.0),
                    rotation: math::Bivector::new(0.0, 0.0, 0.0).exp(), 
                    translation: math::Vector::new(0.0, 0.0, 0.0),
                    translation_velocity: math::Vector::new(0.0, 0.0, 0.0),
                    rotation_velocity: math::Bivector::new(0.0, 0.0, 0.0),
                };
                MAX_PARTICLES_INSTANCE_COUNT
            ],

            current_frame: 0,
        }
    }

    fn renew_swapchain(&mut self) {
        self.cleanup_swapchain();

        (
            self.swapchain, 
            self.swapchain_khr, 
            self.swapchain_images, 
            self.swapchain_image_format, 
            self.swapchain_extent
        ) = swapchain::new_swapchain_and_images(
            &self.instance, 
            self.physical_device, 
            &self.device, 
            &self.surface, 
            self.surface_khr, 
            self.swapchain_extent
        );

        self.swapchain_image_views = swapchain::new_swapchain_image_views(&self.device, &self.swapchain_images, self.swapchain_image_format);

        self.swapchain_framebuffers = swapchain::new_swapchain_framebuffers(&self.device, &self.swapchain_image_views, self.render_pass, self.swapchain_extent);
    }
    
    fn cleanup_swapchain(&mut self) {
        unsafe {
            //TODO:  = no good
            self.device.device_wait_idle().unwrap();

            for i in 0..self.swapchain_images.len() {
                self.device.destroy_framebuffer(self.swapchain_framebuffers[i], None);
                self.device.destroy_image_view(self.swapchain_image_views[i], None);
            }

            self.swapchain.destroy_swapchain(self.swapchain_khr, None);
        }
    }

    fn new_sync_objects(device: &ash::Device
    ) -> (
        Vec<vk::Semaphore>, 
        Vec<vk::Semaphore>,
        Vec<vk::Fence>,
        vk::Fence,
        vk::Fence,
        Vec<vk::Fence>,
    ) {
        let mut image_available_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut render_finished_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut in_flight_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);

        let mut particle_instance_transfer_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
    
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

            particle_instance_transfer_fences.push(
                unsafe { device.create_fence(fence_info, None).unwrap() }
            );
        }

        let vertex_transfer_fence = unsafe { device.create_fence(
            &vk::FenceCreateInfo::builder()
            , None).unwrap() };
        let index_transfer_fence = unsafe { device.create_fence(&vk::FenceCreateInfo::builder()
            , None).unwrap() };

        (
            image_available_semaphores, 
            render_finished_semaphores, 
            in_flight_fences,
            vertex_transfer_fence,
            index_transfer_fence,
            particle_instance_transfer_fences,
        )
    }

    fn new_command_buffers(
        device: &ash::Device, 
        pool: vk::CommandPool,
    ) -> Vec<vk::CommandBuffer> {
        //allocate command command_buffers
        let alloc_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(MAX_FRAMES_IN_FLIGHT as u32);

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

    fn new_render_pass(
        device: &ash::Device, 
        swapchain_image_format: vk::Format,
    ) -> vk::RenderPass {
        let attachment_desc = vk::AttachmentDescription::builder()
            .format(swapchain_image_format)
            .samples(SampleCountFlags::TYPE_1)
            .load_op(AttachmentLoadOp::CLEAR)
            .store_op(AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .build();
        let attachment_descs = [attachment_desc];

        let attachment_ref = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();
        let attachment_refs = [attachment_ref];

        let subpass_desc = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&attachment_refs)
            .build();
        let subpass_descs = [subpass_desc];

        let subpass_dep = vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_subpass(0)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .build();
        let subpass_deps = [subpass_dep];

        let info = vk::RenderPassCreateInfo::builder()
            .subpasses(&subpass_descs)
            .dependencies(&subpass_deps)
            .attachments(&attachment_descs);

        unsafe { 
            device.create_render_pass(&info, None)
                .expect("Failed to create render procedure(renderpass), setup color attachments and sub procedure(subpass) dependencies")
        }
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
        let mut view = math::ModelMat::identity();

        let plane = math::Vector::new(0.0, -1.0, 0.0)
            .wedge(
                &math::Vector::new(self.camera.x_z_angle.sin(), 0.0, self.camera.x_z_angle.cos())
            );

        view
            .translate(-self.camera.x, -self.camera.y, -self.camera.z)
            .rotate(-self.camera.xz_y_angle, plane.yx, plane.zy, plane.xz)
            .rotate(-self.camera.x_z_angle, 0.0, 0.0, 1.0);

        let aspect_ratio = self.swapchain_extent.width as f32 / self.swapchain_extent.height as f32;
        
        let uniform_data = descriptor::UniformData {
            proj_view: math::project(
                view,
                aspect_ratio,
                self.camera.near_z,
                self.camera.far_z,
            ),
        };

        self.uniform_buffer.copy_from_slice::<f32>(&[uniform_data], self.current_frame);
    }

    fn load_particle(
        &mut self, 
        vertices: &[data::VertexData],
        indices: &[u16],
        max_instance_count: usize,
    ) -> usize {
        use instance::BufferSlice;

        let id = self.particles.len();
        let transfer_command_buffer: vk::CommandBuffer = self.transfer_command_buffers[self.current_frame];

        self.particles.push(
            Particle { 
                vertex_slice: BufferSlice {
                    index: self.vertex_count,
                    count: vertices.len(),
                }, 
                index_slice: BufferSlice {
                    index: self.index_count,
                    count: indices.len(),
                },
                instance_slice: BufferSlice {
                    index: self.particle_instance_count,
                    count: 0,
                },
                instance_slice_max_count: max_instance_count,
            }
        );

        self.vertex_buffer.stage_and_copy_from_slice::<f32>(
            vertices,
            self.vertex_count,
            self.transfer_queue,
            self.vertex_transfer_fence,
            transfer_command_buffer,
            &self.physical_device_mem_props,
        );

        self.wait_for_and_reset_fences(&[self.vertex_transfer_fence]);
        self.reset_command_buffer(transfer_command_buffer);

        self.index_buffer.stage_and_copy_from_slice::<u16>(
            indices,
            self.index_count,
            self.transfer_queue,
            self.index_transfer_fence,
            transfer_command_buffer,
            &self.physical_device_mem_props,
        );

        self.wait_for_and_reset_fences(&[self.index_transfer_fence]);
        self.reset_command_buffer(transfer_command_buffer);

        self.vertex_count += vertices.len();
        self.index_count += indices.len();
        self.particle_instance_count += max_instance_count;

        id
    }

    fn unload_particle(&mut self, particle_id: usize) {
        todo!()
    }

    fn load_particle_instances(
        &mut self,
        particle_id: usize,
        particle_instances_count: usize,
    ) {
        let particle = &mut self.particles[particle_id];

        particle.instance_slice.count += particle_instances_count;  
        if particle.instance_slice.count > particle.instance_slice_max_count {
            panic!(
                "Particle instances count {} more than {} allowed for particle {}", 
                particle.instance_slice.count,
                particle.instance_slice_max_count,
                particle_id
            );
        }
    }

    fn unload_particle_instances(
        &mut self,
        particle_id: usize,
        particle_instances_count: usize,
    ) {
        self.particles[particle_id].instance_slice.count -= particle_instances_count;
    }

    fn update_particle_instance_buffer(&mut self) {
        if self.particle_instance_count == 0 { 
            return
        }

        let mut particle_instances_data = Vec::with_capacity(self.particle_instance_count);
        let mut particle_instance_id = 0;
        for p in self.particles.iter() {
            for _ in 0..p.instance_slice_max_count {
                particle_instances_data.push(
                    data::InstanceData {
                        model: self.particle_instances[particle_instance_id].calc_model_mat(),
                    }
                );
                particle_instance_id += 1;
            }
        }

        self.particle_instance_buffer.stage_and_copy_from_slice::<f32>(
            &particle_instances_data, 
            MAX_PARTICLES_INSTANCE_COUNT * self.current_frame,
            self.transfer_queue,
            self.particle_instance_transfer_fences[self.current_frame],
            self.transfer_command_buffers[self.current_frame],
            &self.physical_device_mem_props,
        )
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
                    float32: [0.0, 0.0, 0.0, 1.0],
                }
            }
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
            max_depth: 0.0, 
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

            self.device.cmd_bind_pipeline(
                graphics_command_buffer, 
                vk::PipelineBindPoint::GRAPHICS, 
                self.pipeline
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

            self.device.cmd_bind_vertex_buffers(
                graphics_command_buffer, 
                data::VERTEX_BINDING, 
                &[self.vertex_buffer.handle],
                &[0]
            );
            self.device.cmd_bind_vertex_buffers(
                graphics_command_buffer, 
                data::INSTANCE_BINDING, 
                &[self.particle_instance_buffer.handle], 
                &[0]
            );
            self.device.cmd_bind_index_buffer(
                graphics_command_buffer, 
                self.index_buffer.handle, 
                0, 
                vk::IndexType::UINT16
            );

            self.device.cmd_bind_descriptor_sets(
                graphics_command_buffer, 
                vk::PipelineBindPoint::GRAPHICS, 
                self.pipeline_layout, 
                0, 
                &[self.descriptor_set], 
                &[]
            );

            for particle in self.particles.iter() {
                self.device.cmd_draw_indexed(
                    graphics_command_buffer,
                    particle.index_slice.count as u32, 
                    particle.instance_slice.count as u32,
                    particle.index_slice.index as u32,
                    particle.vertex_slice.index as i32,
                    (particle.instance_slice.index + 
                        self.current_frame * MAX_PARTICLES_INSTANCE_COUNT as usize
                    ) as u32,
                )
            }

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

    fn draw_frame(&mut self) -> bool {
        log::trace!("Drawing frame...");

        let image_available_semaphore = self.image_available_semaphores[self.current_frame];
        let render_finished_semaphore = self.render_finished_semaphores[self.current_frame];
        let in_flight_fence = self.in_flight_fences[self.current_frame];
        let particle_instance_transfer_fence = self.particle_instance_transfer_fences[self.current_frame];

        let graphics_command_buffer = self.graphics_command_buffers[self.current_frame];
        let transfer_command_buffer = self.transfer_command_buffers[self.current_frame];

        self.wait_for_and_reset_fences(&
            if self.particle_instance_count != 0 {
                vec![in_flight_fence, particle_instance_transfer_fence]
            } else {
                vec![in_flight_fence]
            }
        );

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
        if self.particle_instance_count != 0 {
            self.reset_command_buffer(transfer_command_buffer);
        }

        self.update_uniform_buffer();

        self.update_particle_instance_buffer();

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

    pub fn init_game(&mut self) {
        let triangle_vertices = [
            VertexData {
                pos: [-0.5, -0.5, 0.0],
                color: [0.5, 1.0, 0.0],
            },
            VertexData {
                pos: [0.5, -0.5, 0.0],
                color: [0.0, 0.5, 1.0],
            },
            VertexData {
                pos: [0.5, 0.5, 0.0],
                color: [1.0, 0.0, 1.0],
            },
        ];
        let triangle_indices = [
            0, 2, 1,
        ];
    
        let id = self.load_particle(&triangle_vertices, &triangle_indices, 2);
        self.load_particle_instances(id, 2);

        self.particle_instances[0].translation = math::Vector::new(-1.0, 0.0, 0.0);
        self.particle_instances[1].translation = math::Vector::new(1.0, 0.0, 0.0);

    }

    pub fn update_game(&mut self, dt: f32) {
        let coupling = 0.0;

        let d = 
            self.particle_instances[0].translation - 
            self.particle_instances[1].translation;

        let norm_sqr = d.norm_sqr();
        let force = if norm_sqr < 0.2 {
            math::Vector::new(0.0, 0.0, 0.0)
        } else {
            d / norm_sqr / norm_sqr.sqrt() * coupling
        };

        self.particle_instances[0].update_translation_kinematics(-force, dt);
        self.particle_instances[1].update_translation_kinematics(force, dt);
    }

    pub fn handle_input(&mut self, dt: f32) {
        if !self.input_state.is_key_pressed(VirtualKeyCode::Escape) &&
            self.input_state.was_key_pressed(VirtualKeyCode::Escape) {
            self.in_game = !self.in_game;
            self.window.set_cursor_visible(!self.in_game);
            //NOTE: CursorGrabMode::Locked Not implemented by winit
            self.window.set_cursor_grab(
                if self.in_game {
                    CursorGrabMode::Confined
                } else {
                    CursorGrabMode::None
                }
            ).unwrap();

            if !self.in_game {
                self.window.set_cursor_position(
                    PhysicalPosition {
                        x: self.swapchain_extent.width / 2,
                        y: self.swapchain_extent.height / 2,
                    }
                ).unwrap();
            }
        }

        if !self.in_game {
            return;
        }

        let camera = &mut self.camera;

        let dtranslation = camera.translation_speed * dt;
        let drotation = camera.rotation_speed * dt;

        let dc = dtranslation * camera.x_z_angle.cos();
        let ds = dtranslation * camera.x_z_angle.sin();
        if self.input_state.is_key_pressed(VirtualKeyCode::W) {
            camera.z += dc;
            camera.x += ds;
        } 
        if self.input_state.is_key_pressed(VirtualKeyCode::S) {
            camera.z -= dc;
            camera.x -= ds;
        }
        if self.input_state.is_key_pressed(VirtualKeyCode::D) {
            camera.z -= ds;
            camera.x += dc;
        } 
        if self.input_state.is_key_pressed(VirtualKeyCode::A) {
            camera.z += ds;
            camera.x -= dc;
        }

        camera.x_z_angle  += drotation * self.input_state.delta_mouse_pos[0];
        camera.xz_y_angle += drotation * self.input_state.delta_mouse_pos[1];

    }
}

impl Drop for VkApp {
    fn drop(&mut self) {
        log::debug!("Dropping application...");

        self.cleanup_swapchain();

        self.vertex_buffer.destroy();
        self.index_buffer.destroy();
        self.uniform_buffer.destroy();
        self.particle_instance_buffer.destroy();

        unsafe {
            self.device.destroy_descriptor_pool(self.descriptor_pool, None);

            self.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);

            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);

            self.device.destroy_fence(self.vertex_transfer_fence, None);
            self.device.destroy_fence(self.index_transfer_fence, None);
            for i in 0..MAX_FRAMES_IN_FLIGHT {
                self.device.destroy_semaphore(self.image_available_semaphores[i], None);
                self.device.destroy_semaphore(self.render_finished_semaphores[i], None);
                self.device.destroy_fence(self.in_flight_fences[i], None);
                self.device.destroy_fence(self.particle_instance_transfer_fences[i], None);
            }

            self.device.destroy_command_pool(self.transfer_command_pool, None);
            self.device.destroy_command_pool(self.graphics_command_pool, None);

            self.device.destroy_render_pass(self.render_pass, None);

            self.device.destroy_device(None);

            self.surface.destroy_surface(self.surface_khr, None);

            #[cfg(debug_assertions)]
            self.debug_utils.destroy_debug_utils_messenger(self.debug_messenger, None);

            self.instance.destroy_instance(None);
        }
    }
}

fn main() {
    //app init
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Ash Window")
        .with_inner_size(PhysicalSize {width: WIDTH, height: HEIGHT})
        .build(&event_loop)
        .unwrap();
    let mut app = VkApp::new(window);
    app.init_game();

    //running app
    let mut dirty_swapchain = false;
    let mut start_frame_time = 0.0;
    let mut end_frame_time = app.start_instant.elapsed().as_secs_f32();
    let mut dt = end_frame_time;

    use winit::{event_loop::ControlFlow, event::Event};
    event_loop.run(move |system_event, _, control_flow| {
        match system_event {
            Event::MainEventsCleared => {
                //timing
                start_frame_time = end_frame_time;
                end_frame_time = app.start_instant.elapsed().as_secs_f32();
                dt = end_frame_time - start_frame_time;
                
                app.handle_input(dt);
                app.update_game(dt);

                app.input_state.previous_keys_pressed = app.input_state.keys_pressed;
                app.input_state.delta_mouse_pos = [0.0, 0.0];

                if dirty_swapchain {
                    if app.swapchain_extent.width > 0 && app.swapchain_extent.height > 0 {
                        app.renew_swapchain();
                    } else {
                        return;
                    }
                }
                dirty_swapchain = app.draw_frame();
            }
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta, .. } => {
                    app.input_state.delta_mouse_pos = [delta.0 as f32, delta.1 as f32];
                }
                _ => {}
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(v_keycode) = input.virtual_keycode {
                        app.input_state.set_key_pressed(v_keycode, input.state == ElementState::Pressed);
                    }
                }
                WindowEvent::Resized(PhysicalSize {width, height}) => {
                    dirty_swapchain = true;
                    app.swapchain_extent = vk::Extent2D {width, height};
                }
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => {}
            } 
            _ => {}
        }
    })
}
struct Camera {
    x: f32,
    y: f32,
    z: f32,
    x_z_angle: f32,
    xz_y_angle: f32,

    near_z: f32,
    far_z: f32,

    translation_speed: f32,
    rotation_speed: f32,
}