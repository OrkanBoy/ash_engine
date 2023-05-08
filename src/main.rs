pub mod debug;
pub mod data;
pub mod buffer;
pub mod math;
pub mod device;
pub mod swapchain;
pub mod pipeline;
pub mod descriptor;
pub mod instance;

use buffer::Buffer;
use cgmath::Vector3;
use data::VertexData;
use descriptor::UniformBufferObject;
use instance::Particle;
use pipeline::PushConstantData;

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
    window::WindowBuilder, 
    dpi::PhysicalSize, 
    event::{
        WindowEvent,
        VirtualKeyCode,
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

use crate::math::Vector;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;
const MAX_FRAMES_IN_FLIGHT: usize = 2;

struct VkApp {
    camera: Camera,

    start_instant: time::Instant,
    entry: ash::Entry,
    instance: ash::Instance,

    surface: Surface,
    surface_khr: vk::SurfaceKHR,

    debug_utils: DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT, 

    physical_device: vk::PhysicalDevice,
    device: Rc<ash::Device>,

    physical_device_mem_props: vk::PhysicalDeviceMemoryProperties,

    graphics_queue: vk::Queue,
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

    transient_command_pool: vk::CommandPool,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,

    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,

    vertex_buffer: Buffer<data::VertexData>,
    vertex_count: usize,

    index_buffer: Buffer<u16>,
    index_count: usize,

    per_frame_uniform_buffer: Buffer<UniformBufferObject>,

    particles: Vec<instance::Particle>,
    particle_instance_buffer: Buffer<data::InstanceData>,
    particle_instance_count: usize,
    particle_instances: Vec<instance::Instance>,

    current_frame: usize,
}

impl VkApp {
    fn new(window: &winit::window::Window) -> Self {
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
            present_queue
        ) = device::new_logical_device_and_queues(
            &instance, 
            &surface, 
            surface_khr, 
            physical_device,
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

        let transient_command_pool = Self::new_command_pool(
            vk::CommandPoolCreateFlags::TRANSIENT,
            device.clone(), 
            &instance, 
            &surface, 
            surface_khr, 
            physical_device,
        );
        
        const MAX_VERTICES_COUNT: usize = 100;
        const MAX_INDICES_COUNT: usize = 100;
        const MAX_PARTICLES_INSTANCE_COUNT: usize = 40 * 40 * 40;
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
        let per_frame_uniform_buffer = Buffer::new(
            MAX_FRAMES_IN_FLIGHT,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            device.clone(),
            &physical_device_mem_props,
        );
        let particle_instance_buffer = Buffer::new(
            MAX_PARTICLES_INSTANCE_COUNT,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device.clone(),
            &physical_device_mem_props,
        );

        let descriptor_pool = descriptor::new_descriptor_pool(&device, 1);
        let descriptor_set = descriptor::new_descriptor_set(&device, descriptor_pool, descriptor_set_layout, &per_frame_uniform_buffer);

        let command_pool = Self::new_command_pool(
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            device.clone(), 
            &instance, 
            &surface, 
            surface_khr, 
            physical_device,
        );
        let command_buffers = Self::new_command_buffers(
            &device, 
            command_pool, 
        );

        let (image_available_semaphores, render_finished_semaphores, in_flight_fences) = Self::new_sync_objects(&device);

        let camera = Camera {
            x: 0.0, y: 0.0, z: -4.0,
            x_z_angle: 0.0,
            xz_y_angle: 0.0,
            near_z: 1.0,
            far_z: 100.0,
        };
        
        Self {
            camera,

            start_instant: time::Instant::now(),
            entry,
            instance,

            surface,
            surface_khr,

            debug_utils,
            debug_messenger,

            physical_device,
            device,

            physical_device_mem_props,

            graphics_queue,
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

            transient_command_pool,
            command_pool,
            command_buffers,

            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,

            vertex_buffer,
            vertex_count: 0,

            index_buffer,
            index_count: 0,

            per_frame_uniform_buffer,

            particle_instance_buffer, 
            particle_instance_count: 0,
            particles: vec![],
            particle_instances: vec![ //doesn't matter exact value
                instance::Instance {
                    scale: math::Vector::new(1.0, 1.0, 1.0),
                    rotation: math::Bivector::new(0.0, 0.0, 0.0).exp(), 
                    translation: math::Vector::new(0.0, 0.0, 0.0),
                };
                MAX_PARTICLES_INSTANCE_COUNT
            ],

            current_frame: 0,
        }
    }

    fn renew_swapchain(&mut self) {
        unsafe { self.device.device_wait_idle().unwrap(); }

        self.cleanup_swapchain();

        (self.swapchain, self.swapchain_khr, self.swapchain_images, self.swapchain_image_format, self.swapchain_extent) = swapchain::new_swapchain_and_images(&self.instance, self.physical_device, &self.device, &self.surface, self.surface_khr, self.swapchain_extent);

        self.swapchain_image_views = swapchain::new_swapchain_image_views(&self.device, &self.swapchain_images, self.swapchain_image_format);

        self.swapchain_framebuffers = swapchain::new_swapchain_framebuffers(&self.device, &self.swapchain_image_views, self.render_pass, self.swapchain_extent);
    }
    
    fn cleanup_swapchain(&mut self) {
        unsafe {
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
    ) {
        let mut image_available_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut render_finished_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut in_flight_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);

        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            image_available_semaphores.push({
                let info = vk::SemaphoreCreateInfo::builder();
                unsafe { device.create_semaphore(&info, None).unwrap() }
            });
            render_finished_semaphores.push({
                let info = vk::SemaphoreCreateInfo::builder();
                unsafe { device.create_semaphore(&info, None).unwrap() }
            });
            in_flight_fences.push({
                let info = vk::FenceCreateInfo::builder()
                    .flags(vk::FenceCreateFlags::SIGNALED);
                unsafe { device.create_fence(&info, None).unwrap() }
            })
        }
        (image_available_semaphores, render_finished_semaphores, in_flight_fences)
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
        device: Rc<ash::Device>, 
        instance: &ash::Instance,
        surface: &Surface, 
        surface_khr: vk::SurfaceKHR, 
        physical_device: vk::PhysicalDevice
    ) -> vk::CommandPool {
        let (graphics, _) = device::find_queue_families(physical_device, surface, surface_khr, instance);

        let info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(graphics.unwrap())
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
        let elapsed = self.start_instant.elapsed().as_secs_f32();

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
        
        let ubo = UniformBufferObject {
            view_proj: math::project(
                view,
                aspect_ratio,
                self.camera.near_z,
                self.camera.far_z,
            ),
        };
        let ubos = [ubo];

        self.per_frame_uniform_buffer.copy_from_slice::<f32>(&ubos, self.current_frame);
    }

    fn load_particle(
        &mut self, 
        vertices: &[data::VertexData],
        indices: &[u16],
        max_instance_count: usize,
    ) -> usize {
        use instance::BufferSlice;

        let id = self.particles.len();

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
            self.graphics_queue,
            self.transient_command_pool,
            &self.physical_device_mem_props,
        );
        self.index_buffer.stage_and_copy_from_slice::<u16>(
            indices,
            self.index_count,
            self.graphics_queue,
            self.transient_command_pool,
            &self.physical_device_mem_props,
        );

        self.vertex_count += vertices.len();
        self.index_count += indices.len();
        self.particle_instance_count += max_instance_count;

        id
    }

    fn load_particle_instances(
        &mut self,
        particle_id: usize,
        particle_instances_count: usize,
    ) -> Vec<usize> {
        let particle = &mut self.particles[particle_id];

        let old_particle_instance_count = particle.instance_slice.count;

        particle.instance_slice.count += particle_instances_count;  
        if particle.instance_slice.count > particle.instance_slice_max_count {
            panic!(
                "Particle instances count {} exceeded {} allowed for particle {}", 
                particle.instance_slice.count,
                particle.instance_slice_max_count,
                particle_id
            );
        }

        let particle_instance_ids = 
            particle.instance_slice.index + old_particle_instance_count.. 
            particle.instance_slice.index + particle.instance_slice.count;
        particle_instance_ids.collect()
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
            0,
        self.graphics_queue,
            self.transient_command_pool,
            &self.physical_device_mem_props,
        );
    }

    fn record_command_buffer(
        &mut self, 
        command_buffer: vk::CommandBuffer,
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
            self.device.begin_command_buffer(command_buffer, &begin_info).expect("Failed to begin recording command buffer");

            self.device.cmd_begin_render_pass(command_buffer, &render_pass_begin_info, vk::SubpassContents::INLINE);

            self.device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, self.pipeline);

            self.device.cmd_set_viewport(command_buffer, 0, &[viewport]);
            self.device.cmd_set_scissor(command_buffer, 0, &[scissor]);

            self.device.cmd_bind_vertex_buffers(command_buffer, data::VERTEX_BINDING, &[self.vertex_buffer.handle], &[0]);

            self.device.cmd_bind_vertex_buffers(command_buffer, data::INSTANCE_BINDING, &[self.particle_instance_buffer.handle], &[0]);

            self.device.cmd_bind_index_buffer(command_buffer, self.index_buffer.handle, 0, vk::IndexType::UINT16);

            self.device.cmd_bind_descriptor_sets(
                command_buffer, 
                vk::PipelineBindPoint::GRAPHICS, 
                self.pipeline_layout, 
                0, 
                &[self.descriptor_set], 
                &[]
            );

            for particle in self.particles.iter() {
                self.device.cmd_draw_indexed(
                    command_buffer,
                    particle.index_slice.count as u32, 
                    particle.instance_slice.count as u32,
                    particle.index_slice.index as u32,
                    particle.vertex_slice.index as i32,
                    particle.instance_slice.index as u32,
                )
            }

            self.device.cmd_end_render_pass(command_buffer);

            self.device.end_command_buffer(command_buffer).expect("Could not end recording command buffer");
        }
        
    }

    fn draw_frame(&mut self) -> bool {
        log::trace!("Drawing frame...");

        let image_available_semaphore = self.image_available_semaphores[self.current_frame];
        let render_finished_semaphore = self.render_finished_semaphores[self.current_frame];
        let in_flight_fence = self.in_flight_fences[self.current_frame];
        let command_buffer = self.command_buffers[self.current_frame];

        unsafe { 
            self.device.wait_for_fences(&[in_flight_fence], true, u64::MAX).unwrap();
            self.device.reset_fences(&[in_flight_fence]).unwrap();
        }

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

        unsafe { 
            self.device.reset_command_buffer(
                command_buffer, 
                vk::CommandBufferResetFlags::empty()
            ).expect("Failed to reset command buffer contents"); 
        }

        self.record_command_buffer(command_buffer, image_index as usize);

        self.update_uniform_buffer();

        self.update_particle_instance_buffer();

        //render
        {
            let render_info = vk::SubmitInfo::builder()
                .command_buffers(&[command_buffer])
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

        self.vertex_buffer.destroy();
        self.index_buffer.destroy();
        self.per_frame_uniform_buffer.destroy();
        self.particle_instance_buffer.destroy();

        unsafe {
            self.device.destroy_descriptor_pool(self.descriptor_pool, None);

            self.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);

            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);

            for i in 0..MAX_FRAMES_IN_FLIGHT {
                self.device.destroy_semaphore(self.image_available_semaphores[i], None);
                self.device.destroy_semaphore(self.render_finished_semaphores[i], None);
                self.device.destroy_fence(self.in_flight_fences[i], None);
            }

            self.device.destroy_command_pool(self.transient_command_pool, None);
            self.device.destroy_command_pool(self.command_pool, None);

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
    let mut app = VkApp::new(&window);


    let triangle_vertices1 = [
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
    let triangle_indices1 = [
        0, 2, 1,
    ];

    let triangle_vertices2 = [
        VertexData {
            pos: [-0.2, -0.5, 1.0],
            color: [0.3, 1.0, 0.0],
        },
        VertexData {
            pos: [0.5, -0.5, 0.3],
            color: [0.0, 0.3, 1.0],
        },
        VertexData {
            pos: [0.5, 0.5, 0.0],
            color: [0.7, 0.0, 0.2],
        },
    ];
    let triangle_indices2 = [
        0, 2, 1,
    ];

    let particle_cube_size = 15;
    let particle_count = particle_cube_size * particle_cube_size * particle_cube_size;

    let id1 = app.load_particle(&triangle_vertices1, &triangle_indices1, particle_count / 2);
    app.load_particle_instances(id1, particle_count / 2);

    let id2 = app.load_particle(&triangle_vertices2, &triangle_indices2, particle_count / 2);
    app.load_particle_instances(id2, particle_count / 2);

    for x in 0..particle_cube_size {
        let fx = x as f32;
        for y in 0..particle_cube_size {
            let fy = y as f32;
            for z in 0..particle_cube_size {
                let fz = z as f32;

                let p = &mut app.particle_instances[x + particle_cube_size * y + particle_cube_size * particle_cube_size * z];
                p.translation = math::Vector::new(fx, fy, fz);
                p.rotation = math::Bivector::new(fx * 0.1, fy * 0.1, fz * 0.1).exp();
            }
        }
    }


    //running app
    let mut dirty_swapchain = false;
    use winit::{event_loop::ControlFlow, event::Event};

    event_loop.run(move |system_event, _, control_flow| {
        match system_event {
            Event::MainEventsCleared => {
                if dirty_swapchain {
                    if app.swapchain_extent.width > 0 && app.swapchain_extent.height > 0 {
                        app.renew_swapchain();
                    } else {
                        return;
                    }
                }
                dirty_swapchain = app.draw_frame();
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput { input, .. } => {
                    if input.virtual_keycode.is_none() {
                        return;
                    }
                    match input.virtual_keycode.unwrap() {
                        VirtualKeyCode::W => {
                            app.camera.z += 0.1 * app.camera.x_z_angle.cos();
                            app.camera.x += 0.1 * app.camera.x_z_angle.sin();
                        } 
                        VirtualKeyCode::S => {
                            app.camera.z -= 0.1 * app.camera.x_z_angle.cos();
                            app.camera.x -= 0.1 * app.camera.x_z_angle.sin();
                        }
                        VirtualKeyCode::D => {
                            app.camera.z -= 0.1 * app.camera.x_z_angle.sin();
                            app.camera.x += 0.1 * app.camera.x_z_angle.cos();
                        } 
                        VirtualKeyCode::A => {
                            app.camera.z += 0.1 * app.camera.x_z_angle.sin();
                            app.camera.x -= 0.1 * app.camera.x_z_angle.cos();
                        }
                        VirtualKeyCode::Up => {
                            app.camera.xz_y_angle -= 0.01;
                        } 
                        VirtualKeyCode::Down => {
                            app.camera.xz_y_angle += 0.01;
                        }
                        VirtualKeyCode::Right => {
                            app.camera.x_z_angle += 0.01;
                        } 
                        VirtualKeyCode::Left => {
                            app.camera.x_z_angle -= 0.01;
                        }
                        _ => {}
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {

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
}