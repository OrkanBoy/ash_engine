use std::{ffi::CString, io::Read};

use ash::vk;

#[derive(Copy, Clone)]
pub enum Attribute {
    F32x2,
    F32x3,

    // Uses graphics programmer's convention
    // width x height
    F32x4x3,
    F32x3x2,
}

impl Attribute {
    const fn get_total_size(self) -> u32 {
        self.get_total_locations() * self.get_size_of_single_location()
    }

    const fn get_total_locations(self) -> u32 {
        use Attribute::*;

        match self {
            F32x2 | F32x3 => 1,
            F32x4x3 => 4,
            F32x3x2 => 3,
        }
    }

    const fn get_size_of_single_location(self) -> u32 {
        4 * self.get_rgb_components()
    }

    const fn get_rgb_components(self) -> u32 {
        use Attribute::*;

        match self {
            F32x2 | F32x3x2 => 2,
            F32x3 | F32x4x3 => 3,
        }
    }

    const fn get_vk_format_of_single_location(self) -> vk::Format {
        match self.get_rgb_components() {
            2 => vk::Format::R32G32_SFLOAT,
            3 => vk::Format::R32G32B32_SFLOAT,
            _ => panic!(),
        }
    }
}

fn calc_total_stride(attributes: &[Attribute]) -> u32 {
    let mut stride_size = 0;
    for a in attributes {
        stride_size += a.get_total_size();
    }
    stride_size
}

fn calc_total_locations(attributes: &[Attribute]) -> u32 {
    let mut locations = 0;
    for a in attributes {
        locations += a.get_total_locations();
    }
    locations
}

fn push_attrib_descs(
    attrib_descs: &mut Vec<vk::VertexInputAttributeDescription>,
    binding: u32,
    location_offset: u32,
    attributes: &[Attribute],
) -> u32 {
    let mut current_location = location_offset;
    let mut current_offset = 0;
    for attrib in attributes {
        let next_location = current_location + attrib.get_total_locations();

        while current_location < next_location {
            attrib_descs.push(vk::VertexInputAttributeDescription {
                location: current_location,
                binding,
                format: attrib.get_vk_format_of_single_location(),
                offset: current_offset,
            });

            current_offset += attrib.get_size_of_single_location();
            current_location += 1;
        }
    }
    current_location
}

pub const VERTEX_BINDING: u32 = 0;
pub const INSTANCE_BINDING: u32 = 1;

pub fn get_binding_descs(
    vertex_attributes: &[Attribute],
    instance_attributes: &[Attribute],
) -> [vk::VertexInputBindingDescription; 1] {
    [
        vk::VertexInputBindingDescription::builder()
            .binding(VERTEX_BINDING)
            .stride(calc_total_stride(vertex_attributes))
            .input_rate(vk::VertexInputRate::VERTEX)
            .build(),
        // vk::VertexInputBindingDescription::builder()
        //     .binding(INSTANCE_BINDING)
        //     .stride(calc_total_stride(instance_attributes))
        //     .input_rate(vk::VertexInputRate::INSTANCE)
        //     .build(),
    ]
}

pub fn get_attrib_descs(
    vertex_attributes: &[Attribute],
    instance_attributes: &[Attribute],
) -> Vec<vk::VertexInputAttributeDescription> {
    let vertex_locations = calc_total_locations(vertex_attributes);
    let instance_locations = calc_total_locations(instance_attributes);

    let mut attrib_descs = Vec::with_capacity((vertex_locations + instance_locations) as usize);
    let instance_location_offset = push_attrib_descs(
        &mut attrib_descs, 
        VERTEX_BINDING, 
        0, 
        vertex_attributes,
    );
    // push_attrib_descs(
    //     &mut attrib_descs, 
    //     INSTANCE_BINDING, 
    //     instance_location_offset,
    //     instance_attributes
    // );
    attrib_descs
}

fn new_shader_module(
    device: &ash::Device, 
    shader_compiler: &shaderc::Compiler, 
    file_path: &str,
    shader_kind: shaderc::ShaderKind,
) -> vk::ShaderModule {
    let mut file = std::fs::File::open(file_path).unwrap();
    let mut source = String::new();
    file.read_to_string(&mut source).unwrap();

    let code = shader_compiler.compile_into_spirv(
        &source, 
        shader_kind, 
        file_path, 
        "main",
        None,
    ).unwrap().as_binary().to_vec();

    let info = vk::ShaderModuleCreateInfo::builder()
        .code(&code);
    unsafe {
        device
            .create_shader_module(&info, None)
            .unwrap()
    }
}

pub fn new_pipeline_and_layout(
    device: &ash::Device,
    shader_compiler: &shaderc::Compiler,
    render_pass: vk::RenderPass,

    ubo_set_layout: vk::DescriptorSetLayout,
    textures_set_layout: vk::DescriptorSetLayout,

    vertex_shader_path: &str,
    fragment_shader_path: &str,
    
    vertex_attributes: &[Attribute],
    instance_attributes: &[Attribute],
) -> (vk::Pipeline, vk::PipelineLayout) {

    let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::builder()
        .dynamic_states(&[
            vk::DynamicState::VIEWPORT,
            vk::DynamicState::SCISSOR,
        ])
        .build();

    let vert_module = new_shader_module(
        device, 
        &shader_compiler, 
        vertex_shader_path,
        shaderc::ShaderKind::Vertex,
    );
    let frag_module = new_shader_module(
        device, 
        &shader_compiler, 
        fragment_shader_path,
        shaderc::ShaderKind::Fragment,
    );

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

    let binding_descs = get_binding_descs(vertex_attributes, instance_attributes);
    let attrib_descs = get_attrib_descs(vertex_attributes, instance_attributes);
    let vertex_input_create_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&binding_descs)
        .vertex_attribute_descriptions(&attrib_descs)
        .build();

    let input_assembly_create_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false)
        .build();

    let viewport_create_info = vk::PipelineViewportStateCreateInfo::builder()
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

    let depth_stencil_info = vk::PipelineDepthStencilStateCreateInfo::builder()
        .depth_test_enable(true)
        .depth_write_enable(true)
        .depth_compare_op(vk::CompareOp::LESS)
        .depth_bounds_test_enable(false)
        .min_depth_bounds(0.0)
        .max_depth_bounds(1.0)
        .stencil_test_enable(false)
        .front(Default::default())
        .back(Default::default())
        .build();

    let layout = {
        let layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&[
                ubo_set_layout,
                // textures_set_layout,
            ])
            .build();

        unsafe { device.create_pipeline_layout(&layout_info, None).unwrap() }
    };

    let info = vk::GraphicsPipelineCreateInfo::builder()
        .dynamic_state(&dynamic_state_info)
        .stages(&[vert_stage_info, frag_stage_info])
        .vertex_input_state(&vertex_input_create_info)
        .input_assembly_state(&input_assembly_create_info)
        .viewport_state(&viewport_create_info)
        .rasterization_state(&rasterizer_create_info)
        .multisample_state(&multisampling_create_info)
        .depth_stencil_state(&depth_stencil_info)
        .color_blend_state(&color_blending_info)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0) // what does this do?!
        .build();
    let pipeline = unsafe {
        device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
            .unwrap()[0]
    };

    unsafe {
        device.destroy_shader_module(vert_module, None);
        device.destroy_shader_module(frag_module, None);
    };

    (pipeline, layout)
}
