use std::ffi::CString;

use ash::vk;

use crate::vertex;

pub fn new_pipeline(
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

    let vert_module = unsafe { 
        device.create_shader_module(&vk::ShaderModuleCreateInfo::builder()
            .code(vert_code), 
            None
        ).unwrap()
    };
    let frag_module = unsafe { 
        device.create_shader_module(&vk::ShaderModuleCreateInfo::builder()
            .code(frag_code), 
            None
        ).unwrap()
    };

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
        .vertex_binding_descriptions(&vertex::get_binding_descs())
        .vertex_attribute_descriptions(&vertex::get_attrib_descs())
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