pub mod debug;
pub mod vertex;
pub mod buffer;

use buffer::Buffer;
use vertex::Vertex;
use cgmath::{Deg, Matrix4, Point3, Vector3, SquareMatrix};

use raw_window_handle::{
    HasRawDisplayHandle, 
    HasRawWindowHandle,
};
use std::{ffi::{
    CString, 
    CStr,
}, rc::Rc, time};

use winit::{
    self, 
    event_loop::EventLoop, 
    window::WindowBuilder, 
    dpi::PhysicalSize, 
    event::WindowEvent
};

use ash::{
    vk::{
        self, 
        SampleCountFlags, 
        AttachmentLoadOp, 
        AttachmentStoreOp, 
        CommandPoolCreateFlags, 
        CommandBufferUsageFlags, PhysicalDeviceMemoryProperties, BufferUsageFlags,
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
const VERTICES: [Vertex; 4] = [
    Vertex {
        pos: [-0.5, -0.5],
        color: [1.0, 0.3, 0.0],
    },
    Vertex {
        pos: [0.5, -0.5],
        color: [0.0, 0.4, 0.0],
    },
    Vertex {
        pos: [0.5, 0.5],
        color: [0.1, 0.0, 0.5],
    },
    Vertex {
        pos: [-0.5, 0.5],
        color: [0.7, 0.0, 1.0],
    },
];
const INDICES: [u16; 6] = [0, 2, 1, 0, 3, 2];

struct VkApp {
    start_instant: time::Instant,
    entry: ash::Entry,
    instance: ash::Instance,

    surface: Surface,
    surface_khr: vk::SurfaceKHR,

    debug_utils: DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT, 

    physical_device: vk::PhysicalDevice,
    device: Rc<ash::Device>,
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
    descriptor_sets: Vec<vk::DescriptorSet>,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,

    transient_command_pool: vk::CommandPool,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,

    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,

    vertex_buffer: Buffer,
    index_buffer: Buffer,
    uniform_buffers: Vec<Buffer>,

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

        let physical_device = Self::pick_physical_device(
            &instance, 
            &surface, 
            surface_khr,
        );

        let (device, 
            graphics_queue, 
            present_queue
        ) = Self::new_logical_device_and_queues(
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
        ) = Self::new_swapchain_and_images(
            &instance, 
            physical_device, 
            &device, 
            &surface, 
            surface_khr, 
            vk::Extent2D{width: WIDTH, height: HEIGHT}
        );

        let swapchain_image_views = Self::new_swapchain_image_views(
            &device, 
            &swapchain_images, 
            swapchain_image_format,
        );

        let render_pass = Self::new_render_pass(
            &device, 
            swapchain_image_format,
        );

        let descriptor_set_layout = Self::new_descriptor_set_layout(&device);
        let (pipeline, pipeline_layout) = Self::new_pipeline(
            &device, 
            swapchain_extent, 
            render_pass,
            descriptor_set_layout,
        );

        let swapchain_framebuffers = Self::new_swapchain_framebuffers(
            &device, 
            &swapchain_image_views, 
            render_pass, 
            swapchain_extent,
        );

        let memory_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let transient_command_pool = Self::new_command_pool(
            vk::CommandPoolCreateFlags::TRANSIENT,
            device.clone(), 
            &instance, 
            &surface, 
            surface_khr, 
            physical_device,
        );
        
        let vertex_buffer = Buffer::new_local_with_data::<u32, _>(
            &VERTICES,
            BufferUsageFlags::VERTEX_BUFFER,
            graphics_queue,
            transient_command_pool,
            device.clone(),
            &memory_props,
        );
        let index_buffer = Buffer::new_local_with_data::<u16, _>(
            &INDICES,
            BufferUsageFlags::INDEX_BUFFER,
            graphics_queue,
            transient_command_pool,
            device.clone(),
            &memory_props,
        );
        let mut uniform_buffers = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        for _ in 0..uniform_buffers.capacity() {
            uniform_buffers.push(
                Buffer::new(
                    std::mem::size_of::<UniformBufferObject>() as vk::DeviceSize,
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                    device.clone(),
                    &memory_props,
                )
            );
        }
        let descriptor_pool = Self::new_descriptor_pool(&device, MAX_FRAMES_IN_FLIGHT as u32);
        let descriptor_sets = Self::new_descriptor_sets(&device, descriptor_pool, descriptor_set_layout, &uniform_buffers);

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

        let (image_available_semaphores, render_finished_semaphores, in_flight_fences) = Self::new_sync_objetcs(&device);

        Self {
            start_instant: time::Instant::now(),
            entry,
            instance,

            surface,
            surface_khr,

            debug_utils,
            debug_messenger,

            physical_device,
            device,
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
            descriptor_sets,
            pipeline_layout,
            pipeline,

            transient_command_pool,
            command_pool,
            command_buffers,

            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,

            vertex_buffer,
            index_buffer,
            uniform_buffers,

            current_frame: 0,
        }
    }

    fn new_descriptor_pool(device: &ash::Device, size: u32) -> vk::DescriptorPool {
        let pool_size = vk::DescriptorPoolSize::builder()
            .ty(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(size)
            .build();
        let pool_sizes = [pool_size];

        let info = vk::DescriptorPoolCreateInfo::builder()
            .max_sets(size)
            .pool_sizes(&pool_sizes);

        unsafe { device.create_descriptor_pool(&info, None).expect("Failed to create descriptor pool") }
    }

    fn new_descriptor_sets(
        device: &ash::Device,
        pool: vk::DescriptorPool,
        layout: vk::DescriptorSetLayout,
        uniform_buffers: &[Buffer],
    ) -> Vec<vk::DescriptorSet> {
        let layouts = (0..uniform_buffers.len())
            .map(|_| layout)
            .collect::<Vec<_>>();
        let alloc_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(pool)
            .set_layouts(&layouts);
        let sets = unsafe {
            device.allocate_descriptor_sets(&alloc_info).expect("Failed to create descriptor sets")
        };

        for (set, buffer) in sets.iter().zip(uniform_buffers.iter()) {
            let buffer_info = vk::DescriptorBufferInfo::builder()
                .buffer(buffer.handle)
                .offset(0)
                .range(std::mem::size_of::<UniformBufferObject>() as vk::DeviceSize)
                .build();
            let buffer_infos = [buffer_info];

            let write = vk::WriteDescriptorSet::builder()
                .dst_set(*set)
                .dst_array_element(0)
                .dst_binding(0)
                .buffer_info(&buffer_infos)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .build();
            let writes = [write];

            unsafe { device.update_descriptor_sets(&writes, &[]) }
        }
        sets
    }

    fn new_descriptor_set_layout(device: &ash::Device) -> vk::DescriptorSetLayout {
        let bindings = [
            vk::DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .stage_flags(vk::ShaderStageFlags::VERTEX)
                .build(),
        ];

        let info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&bindings);

        unsafe { device.create_descriptor_set_layout(&info, None).expect("Failed to create descriptor set layouts") }
    }

    fn renew_swapchain(&mut self) {
        unsafe { self.device.device_wait_idle().unwrap(); }

        self.cleanup_swapchain();

        (self.swapchain, self.swapchain_khr, self.swapchain_images, self.swapchain_image_format, self.swapchain_extent) = Self::new_swapchain_and_images(&self.instance, self.physical_device, &self.device, &self.surface, self.surface_khr, self.swapchain_extent);

        self.swapchain_image_views = Self::new_swapchain_image_views(&self.device, &self.swapchain_images, self.swapchain_image_format);

        self.swapchain_framebuffers = Self::new_swapchain_framebuffers(&self.device, &self.swapchain_image_views, self.render_pass, self.swapchain_extent);
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

    fn new_sync_objetcs(device: &ash::Device
    ) -> (
        Vec<vk::Semaphore>, 
        Vec<vk::Semaphore>,
        Vec<vk::Fence>,
    ) {
        let mut image_available_semaphores = vec![];
        let mut render_finished_semaphores = vec![];
        let mut in_flight_fences = vec![];

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
        let (graphics, _) = Self::find_queue_families(physical_device, surface, surface_khr, instance);

        let info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(graphics.unwrap())
            .flags(create_flags);

        unsafe { device.create_command_pool(&info, None).expect("Failed to create command pool") }
    }

    fn new_swapchain_framebuffers(
        device: &ash::Device, 
        image_views: &[vk::ImageView], 
        render_pass: vk::RenderPass, 
        extent: vk::Extent2D,
    ) -> Vec<vk::Framebuffer> {
        image_views
            .iter()
            .map(|view|
                {
                    let attachments = [*view];
                    let info = vk::FramebufferCreateInfo::builder()
                        .attachments(&attachments)
                        .render_pass(render_pass)
                        .width(extent.width)
                        .height(extent.height)
                        .layers(1);

                    unsafe { device.create_framebuffer(&info, None).unwrap() }
                }
            )
            .collect()
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

        unsafe { device.create_render_pass(&info, None).expect("Failed to create render procedure(renderpass), setup color attachments and sub procedure(subpass) dependencies") }
    }

    fn new_pipeline(
        device: &ash::Device,
        swapchain_extent: vk::Extent2D,
        render_pass: vk::RenderPass,
        descriptor_set_layout: vk::DescriptorSetLayout,
    ) -> (vk::Pipeline, vk::PipelineLayout) {
        let dynamic_states = [
            vk::DynamicState::VIEWPORT,
            vk::DynamicState::SCISSOR,
        ];

        let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(&dynamic_states)
            .build();

        let vert_code = vk_shader_macros::include_glsl!("shaders/foo.vert");
        let frag_code = vk_shader_macros::include_glsl!("shaders/foo.frag");

        let vert_module = Self::new_shader_module(device, vert_code);
        let frag_module = Self::new_shader_module(device, frag_code);

        let entry_name = CString::new("main").unwrap();
        let vert_stage_info = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert_module)
            .name(&entry_name)
            .build();
        let frag_stage_info = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag_module)
            .name(&entry_name)
            .build();
        let shader_stage_infos = [vert_stage_info, frag_stage_info];


        let vertex_input_create_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&Vertex::get_binding_descs())
            .vertex_attribute_descriptions(&Vertex::get_attrib_descs())
            .build();

        let input_assembly_create_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false)
            .build();

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: swapchain_extent.width as _,
            height: swapchain_extent.height as _,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let viewports = [viewport];
        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: swapchain_extent,
        };
        let scissors = [scissor];
        let viewport_create_info = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&viewports)
            .scissors(&scissors)
            .build();

        let rasterizer_create_info = vk::PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false)
            .depth_bias_constant_factor(0.0)
            .depth_bias_clamp(0.0)
            .depth_bias_slope_factor(0.0)
            .build();

        let multisampling_create_info = vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .min_sample_shading(1.0)
            // .sample_mask() // null
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false)
            .build();

        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ZERO)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .build();
        let color_blend_attachments = [color_blend_attachment];

        let color_blending_info = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .attachments(&color_blend_attachments)
            .blend_constants([0.0, 0.0, 0.0, 0.0])
            .build();

        let descriptor_set_layouts = [descriptor_set_layout];
        let layout = {
            let layout_info = vk::PipelineLayoutCreateInfo::builder()
                .set_layouts(&descriptor_set_layouts);
                // .push_constant_ranges


            unsafe {
                device
                    .create_pipeline_layout(&layout_info, None)
                    .unwrap()
            }
        };

        let info = vk::GraphicsPipelineCreateInfo::builder()
            .dynamic_state(&dynamic_state_info)
            .stages(&shader_stage_infos)
            .vertex_input_state(&vertex_input_create_info)
            .input_assembly_state(&input_assembly_create_info)
            .viewport_state(&viewport_create_info)
            .rasterization_state(&rasterizer_create_info)
            .multisample_state(&multisampling_create_info)
            .color_blend_state(&color_blending_info)
            .layout(layout)
            .render_pass(render_pass)
            .subpass(0)
            .build();
        let pipeline = unsafe { device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).unwrap()[0] };

        unsafe {
            device.destroy_shader_module(vert_module, None);
            device.destroy_shader_module(frag_module, None);
        };


        (pipeline, layout)
    }

    fn new_shader_module(device: &ash::Device, src_code: &[u32]) -> vk::ShaderModule {
        let info = vk::ShaderModuleCreateInfo::builder()
            .code(src_code);

        unsafe { device.create_shader_module(&info, None).unwrap() }
    }

    fn new_swapchain_image_views(
        device: &ash::Device, 
        images: &[vk::Image], 
        format: vk::Format) -> Vec<vk::ImageView> {
        images.iter()
            .map(|image| {
                let info = vk::ImageViewCreateInfo::builder()
                    .image(*image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .build();
                
                unsafe { device.create_image_view(&info, None).unwrap() }
            })
            .collect()
    }

    fn new_swapchain_and_images(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        surface: &Surface,
        surface_khr: vk::SurfaceKHR,
        preferred_swapchain_extent: vk::Extent2D,
    ) -> (
        Swapchain,
        vk::SwapchainKHR,
        Vec<vk::Image>,
        vk::Format,
        vk::Extent2D,
        ) {
        let (capabilities, formats, present_modes) = unsafe {
            (
                surface.get_physical_device_surface_capabilities(physical_device, surface_khr).unwrap(),
                surface.get_physical_device_surface_formats(physical_device, surface_khr).unwrap(),
                surface.get_physical_device_surface_present_modes(physical_device, surface_khr).unwrap(),
            )
        };

        let format = Self::choose_swapchain_format(&formats);
        let present_mode = Self::choose_swapchain_present_mode(&present_modes);
        let extent = Self::choose_swapchain_extent(&capabilities, preferred_swapchain_extent);
        let image_count = (capabilities.min_image_count + 1).min(capabilities.max_image_count);

        log::debug!(
            "Creating swapchain.\n\tFormat: {:?}\n\tColorSpace: {:?}\n\tPresentMode: {:?}\n\tExtent: {:?}\n\tImageCount: {:?}",
            format.format,
            format.color_space,
            present_mode,
            extent,
            image_count,
        );

        let (graphics, present) = Self::find_queue_families(physical_device, surface, surface_khr, instance);
        let family_indices = [graphics.unwrap(), present.unwrap()];

        let info = {
            let mut builder = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface_khr)
                .min_image_count(image_count)
                .image_format(format.format)
                .image_color_space(format.color_space)
                .image_extent(extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT);

            builder = match (graphics, present) {
                (Some(graphics), Some(present)) if graphics != present => builder
                    .image_sharing_mode(vk::SharingMode::CONCURRENT)
                    .queue_family_indices(&family_indices),
                (Some(_), Some(_)) => builder.image_sharing_mode(vk::SharingMode::EXCLUSIVE),
                _ => panic!(),
            };

            builder
                .pre_transform(capabilities.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .build()
        };

        let swapchain = Swapchain::new(instance, device);
        let swapchain_khr = unsafe { swapchain.create_swapchain(&info, None).unwrap() };
        let images = unsafe { swapchain.get_swapchain_images(swapchain_khr).unwrap() } ;

        (swapchain, swapchain_khr, images, format.format, extent)
    }

    fn choose_swapchain_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
        if formats.len() == 1 && formats[0].format == vk::Format::UNDEFINED {
            return vk::SurfaceFormatKHR {
                format: vk::Format::B8G8R8A8_UNORM,
                color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            }
        }

        *formats
            .iter()
            .find(|f| 
                f.format == vk::Format::B8G8R8A8_UNORM && f.color_space ==  vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .unwrap_or(&formats[0])
    }

    fn choose_swapchain_present_mode(present_modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
        if present_modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else if present_modes.contains(&vk::PresentModeKHR::FIFO) {
            vk::PresentModeKHR::FIFO
        } else {
            vk::PresentModeKHR::IMMEDIATE
        }
    }

    fn choose_swapchain_extent(capabilities: &vk::SurfaceCapabilitiesKHR, preferred_swapchain_extent: vk::Extent2D) -> vk::Extent2D {
        if capabilities.current_extent.width != u32::MAX {
            return capabilities.current_extent;
        }
        
        let min = capabilities.min_image_extent;
        let max = capabilities.max_image_extent;
        let width = preferred_swapchain_extent.width.min(max.width).max(min.width);
        let height = preferred_swapchain_extent.height.min(max.height).max(min.height);
        vk::Extent2D { width, height }
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
        let (_, layer_name_ptrs) = &debug::get_c_layer_names_and_ptrs();

        let mut info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_name_ptrs);
            
        #[cfg(debug_assertions)] {
            debug::check_validation_layer_support(entry);
            info = info.enabled_layer_names(&layer_name_ptrs);
        }

        unsafe { entry.create_instance(&info, None).unwrap() }
    }

    fn pick_physical_device(
        instance: &ash::Instance, 
        surface: &Surface, 
        surface_khr: vk::SurfaceKHR) -> vk::PhysicalDevice {
        let physical_device = unsafe{instance.enumerate_physical_devices()}
            .unwrap()
            .into_iter()
            .find(|device| Self::is_physical_device_suitable(*device, surface, surface_khr, &instance))
            .unwrap();

        let props = unsafe {instance.get_physical_device_properties(physical_device)};
        log::debug!("Selected physical device: {:?}", unsafe{CStr::from_ptr(props.device_name.as_ptr())});
        physical_device
    }

    fn is_physical_device_suitable(
        physical_device: vk::PhysicalDevice, 
        surface: &Surface, 
        surface_khr: vk::SurfaceKHR, 
        instance: &ash::Instance) -> bool {
        let (graphics, present) = Self::find_queue_families(physical_device, surface, surface_khr, &instance);
        graphics.is_some() && present.is_some()
    }

    fn find_queue_families(
        physical_device: vk::PhysicalDevice,
        surface: &Surface,
        surface_khr: vk::SurfaceKHR,
        instance: &ash::Instance) -> (Option<u32>, Option<u32>) {
        let props = unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

        let mut graphics_index = None;
        let mut present_index = None;

        for (index, family) in props.iter().filter(|p| p.queue_count > 0).enumerate() {
            let index = index as u32;

            if graphics_index.is_none() && family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                graphics_index = Some(index);
            }

            let present_support = unsafe { surface.get_physical_device_surface_support(physical_device, index, surface_khr) }.unwrap();
            if present_index.is_none() && present_support {
                present_index = Some(index);
            }

            if graphics_index.is_some() && present_index.is_some() {
                break;
            }
        }
        (graphics_index, present_index)
    }
    
    fn new_logical_device_and_queues(
        instance: &ash::Instance, 
        surface: &Surface,
        surface_khr: vk::SurfaceKHR,
        physical_device: vk::PhysicalDevice
    ) -> (Rc<ash::Device>, vk::Queue, vk::Queue) {

        let (graphics_family_index, present_family_index) = Self::find_queue_families(physical_device, surface, surface_khr, instance);
        let graphics_family_index = graphics_family_index.unwrap();
        let present_family_index = present_family_index.unwrap();
        
        let queue_priorities = [1.0];

        let queue_infos = {
            let mut indices = vec![graphics_family_index, present_family_index];
            indices.dedup();

            indices.iter().map(|&index| vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(index)
                .queue_priorities(&queue_priorities)
                .build())
            .collect::<Vec<_>>()
        };

        let (_, layer_name_ptrs) = &debug::get_c_layer_names_and_ptrs();

        let physical_device_features = vk::PhysicalDeviceFeatures::builder();
        let (_, device_extension_name_ptrs) = &Self::get_c_device_extension_names_and_ptrs();

        Self::check_device_extension_support(&instance, physical_device);

        let mut info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_infos)
            .enabled_features(&physical_device_features)
            .enabled_extension_names(&device_extension_name_ptrs);

        #[cfg(debug_assertions)] {
            info = info.enabled_layer_names(&layer_name_ptrs);
        }

        unsafe {
            let device = instance.create_device(physical_device, &info, None).unwrap();
            let graphics_queue = device.get_device_queue(graphics_family_index, 0);
            let present_queue = device.get_device_queue(present_family_index, 0);

            (Rc::from(device), graphics_queue, present_queue)
        }
    }

    fn check_device_extension_support(instance: &ash::Instance, physical_device: vk::PhysicalDevice) {
        let (required_extensions, _) = &Self::get_c_device_extension_names_and_ptrs();

        let extension_props = unsafe {
            instance
                .enumerate_device_extension_properties(physical_device)
                .unwrap()
        };

        for required in required_extensions.iter() {
            let found = extension_props.iter().any(|ext| {
                let name = unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) };
                required == &name
            });

            if !found {
                panic!("Could not find required device extension {:?}", required)
            }
        }
    }

    fn get_c_device_extension_names_and_ptrs() -> (Vec<&'static CStr>, Vec<*const i8>) {
        let c_device_extension_names = vec![Swapchain::name()];
        let device_extension_name_ptrs = c_device_extension_names.iter()
            .map(|name| name.as_ptr())
            .collect::<Vec<_>>();

        (c_device_extension_names, device_extension_name_ptrs)
    }

    fn update_uniform_buffer(&mut self) {
        let elapsed = self.start_instant.elapsed().as_secs_f32();

        let (s, c) = f32::sin_cos(elapsed);
        let ubo = UniformBufferObject {
            model: Matrix4::new( //camera model inverse
                c, s, 0.0, 0.0,
                -s, c, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ),
            view: Matrix4::identity(),
            proj: Matrix4::new(
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ),
        };
        let ubos = [ubo];

        self.uniform_buffers[self.current_frame].copy_from_slice::<f32, _>(&ubos);
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
            self.device.begin_command_buffer(command_buffer, &begin_info).unwrap();

            self.device.cmd_begin_render_pass(command_buffer, &render_pass_begin_info, vk::SubpassContents::INLINE);

            self.device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, self.pipeline);

            self.device.cmd_set_viewport(command_buffer, 0, &[viewport]);
            self.device.cmd_set_scissor(command_buffer, 0, &[scissor]);

            self.device.cmd_bind_vertex_buffers(command_buffer, 0, &[self.vertex_buffer.handle], &[0]);
            self.device.cmd_bind_index_buffer(command_buffer, self.index_buffer.handle, 0, vk::IndexType::UINT16);
            self.device.cmd_bind_descriptor_sets(command_buffer, vk::PipelineBindPoint::GRAPHICS, self.pipeline_layout, 0, &self.descriptor_sets[image_index..=image_index], &[]);

            self.device.cmd_draw_indexed(command_buffer, INDICES.len() as u32, 1, 0, 0, 0);

            self.device.cmd_end_render_pass(command_buffer);

            self.device.end_command_buffer(command_buffer).unwrap();
        }
        
    }

    fn draw_frame(&mut self) -> bool {
        log::trace!("Drawing frame...");

        let image_available_semaphore = self.image_available_semaphores[self.current_frame];
        let render_finished_semaphore = self.render_finished_semaphores[self.current_frame];
        let in_flight_fence = self.in_flight_fences[self.current_frame];
        let command_buffer = self.command_buffers[self.current_frame];

        let wait_fences = [in_flight_fence];
        unsafe { 
            self.device.wait_for_fences(&wait_fences, true, u64::MAX).unwrap();
            self.device.reset_fences(&wait_fences).unwrap();
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

        unsafe { self.device.reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty()).unwrap(); }
        self.record_command_buffer(command_buffer, image_index as usize);

        self.update_uniform_buffer();

        let wait_semaphores = [image_available_semaphore];
        let signal_semaphores = [render_finished_semaphore];

        //render
        {
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let command_buffers = [command_buffer];
            let render_info = vk::SubmitInfo::builder()
                .command_buffers(&command_buffers)
                .wait_dst_stage_mask(&wait_stages)
                .wait_semaphores(&wait_semaphores)
                .signal_semaphores(&signal_semaphores)
                .build();
            let render_infos = [render_info];

            unsafe { self.device.queue_submit(self.graphics_queue, &render_infos, in_flight_fence).unwrap(); }
        }

        let swapchain_khrs = [self.swapchain_khr];
        let image_indices = [image_index];

        //present
        {
            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&signal_semaphores)
                .swapchains(&swapchain_khrs)
                .image_indices(&image_indices)
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
        for ub in self.uniform_buffers.iter_mut() { ub.destroy(); }

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
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Ash Window")
        .with_inner_size(PhysicalSize {width: WIDTH, height: HEIGHT})
        .build(&event_loop)
        .unwrap();

    let mut app = VkApp::new(&window);
    let mut dirty_swapchain = false;

    use winit::{event_loop::ControlFlow, event::Event};

    //TODO: update swapchain brief period after resizing stopped
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
                WindowEvent::Resized(PhysicalSize {width, height}) => {
                    dirty_swapchain = true;
                    app.swapchain_extent = vk::Extent2D {width, height};
                }
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => {},
            } 
            _ => {},
        }
    })
}

//TODO: update to projective geometric algebra
#[derive(Clone, Copy)]
struct UniformBufferObject {
    model: Matrix4<f32>,
    view: Matrix4<f32>,
    proj: Matrix4<f32>
}
