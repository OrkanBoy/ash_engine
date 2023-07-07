use std::rc::Rc;

use ash::vk;

pub enum TextureType {
    Diffuse,
    Specular,
    Height,
    Normal,
}

pub struct Texture {
    device: Rc<ash::Device>,

    width: u32,
    height: u32,
    ty: TextureType,

    image: vk::Image,
    pub image_view: vk::ImageView,
    pub sampler: vk::Sampler,
    memory: vk::DeviceMemory,
}

impl Texture {
    pub fn load(
        path: &str,
        device: Rc<ash::Device>,
        physical_device_memory_properties: vk::PhysicalDeviceMemoryProperties,
        ty: TextureType,
        transition_command_pool: vk::CommandPool,
        transition_queue: vk::Queue,
        transition_family_index: u32,
    ) -> Texture {
        let image = image::open(path).unwrap(); //TODO: implement own image reader
        let image_as_rgb = image.to_rgba();
        let image_width = (&image_as_rgb).width();
        let image_height = (&image_as_rgb).height();
        let pixels = image_as_rgb.into_raw();

        let mut staging_buffer = super::buffer::Buffer::new(
            pixels.len() as vk::DeviceSize,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            device.clone(),
            &physical_device_memory_properties,
        );

        staging_buffer.copy_from_slice(&pixels, 0);

        let mut texture = Self::new(
            device.clone(),
            &physical_device_memory_properties,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            ty,
            image_width,
            image_height,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
        );

        crate::VkApp::execute_transient_commands(
            &device, 
            transition_command_pool, 
            transition_queue, 
            |transition_command_buffer| {
                super::image::cmd_transition_image_layout(
                    &device,
                    texture.image,
                    transition_command_buffer,
                    transition_family_index,
                    vk::Format::R8G8B8A8_UNORM,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                );
    
                texture.cmd_copy_from_buffer(transition_command_buffer, &staging_buffer);
    
                super::image::cmd_transition_image_layout(
                    &device,
                    texture.image,
                    transition_command_buffer,
                    transition_family_index,
                    vk::Format::R8G8B8A8_UNORM,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                );
            }
        );

        unsafe {
            staging_buffer.destroy();
        }

        texture
    }

    pub fn new(
        device: Rc<ash::Device>,
        physical_device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
        memory_properties: vk::MemoryPropertyFlags,
        ty: TextureType,
        width: u32,
        height: u32,
        format: vk::Format,
        tiling: vk::ImageTiling,
        usage: vk::ImageUsageFlags,
    ) -> Self {
        let (image, memory) = super::image::new_image_and_memory(
            &device,
            physical_device_memory_properties,
            width,
            height,
            usage,
            format,
            tiling,
            memory_properties,
        );

        let image_view =
            super::image::new_image_view(&device, image, format, vk::ImageAspectFlags::COLOR);

        let sampler = {
            let info = vk::SamplerCreateInfo::builder()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::REPEAT)
                .address_mode_v(vk::SamplerAddressMode::REPEAT)
                .address_mode_w(vk::SamplerAddressMode::REPEAT)
                .anisotropy_enable(true)
                .max_anisotropy(16.0)
                .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
                .unnormalized_coordinates(false)
                .compare_enable(false)
                .compare_op(vk::CompareOp::ALWAYS)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                .mip_lod_bias(0.0)
                .min_lod(0.0)
                .max_lod(0.0);

            unsafe { device.create_sampler(&info, None).unwrap() }
        };

        Self {
            device,

            width,
            height,
            ty,

            image,
            image_view,
            sampler,
            memory,
        }
    }

    pub fn cmd_copy_from_buffer(
        &mut self,
        command_buffer: vk::CommandBuffer,
        buffer: &super::buffer::Buffer,
    ) {
        let region = vk::BufferImageCopy::builder()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D {
                width: self.width,
                height: self.height,
                depth: 1,
            })
            .build();
        let regions = [region];

        unsafe {
            self.device.cmd_copy_buffer_to_image(
                command_buffer,
                buffer.handle,
                self.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
            )
        }
    }
    // caller must ensure only called once
    pub unsafe fn destroy(&mut self) {
        self.device.destroy_sampler(self.sampler, None);
        self.device.destroy_image_view(self.image_view, None);
        self.device.destroy_image(self.image, None);
        self.device.free_memory(self.memory, None);
    }
}