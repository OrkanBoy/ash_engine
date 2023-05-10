use ash::{vk, extensions::khr::{Swapchain, Surface}};

use crate::device;

pub fn new_swapchain_and_images(
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

    let format = choose_swapchain_format(&formats);
    let present_mode = choose_swapchain_present_mode(&present_modes);
    let extent = choose_swapchain_extent(&capabilities, preferred_swapchain_extent);
    let image_count = (capabilities.min_image_count + 1).min(capabilities.max_image_count);

    log::debug!(
        "Creating swapchain.\n\tFormat: {:?}\n\tColorSpace: {:?}\n\tPresentMode: {:?}\n\tExtent: {:?}\n\tImageCount: {:?}",
        format.format,
        format.color_space,
        present_mode,
        extent,
        image_count,
    );

    let info = {
        let mut builder = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface_khr)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT);

        let (
            graphics, 
            _,
            present
        ) = device::find_queue_family_indices(physical_device, &surface, surface_khr, &instance);

        let graphics = graphics.unwrap();
        let present = present.unwrap();

        let indices = [graphics, present];
        builder = 
            if graphics != present { 
                builder
                    .image_sharing_mode(vk::SharingMode::CONCURRENT)
                    .queue_family_indices(&indices)
            } else {
                builder.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
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

pub fn new_swapchain_image_views(
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

pub fn new_swapchain_framebuffers(
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