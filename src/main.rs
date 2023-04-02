pub mod vk_logger;
pub mod debug;

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use vk_logger::VkLogger;

use std::{ffi::{
    CString, CStr,
}, mem::swap};

use winit::{self, event_loop::EventLoop};

use ash::{
    vk,
    extensions::{khr::{Surface, Win32Surface, Swapchain}, ext::{DebugUtils}},
};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

struct VkApp {
    entry: ash::Entry,
    instance: ash::Instance,

    window: winit::window::Window,
    event_loop: winit::event_loop::EventLoop<()>,
    surface: Surface,
    surface_khr: vk::SurfaceKHR,

    debug_utils: DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT, 

    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,

    swapchain: Swapchain, 
    swapchain_khr: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_format: vk::Format,
    swapchain_extent: vk::Extent2D,
}

impl VkApp {
    fn new() -> Self {
        log::debug!("Creating app...");

        let event_loop = EventLoop::new();
        let window = winit::window::Window::new(&event_loop).unwrap();

        let entry = ash::Entry::linked();
        let instance = Self::new_instance(&entry);

        let surface = Surface::new(&entry, &instance);
        let surface_khr = unsafe { ash_window::create_surface(&entry, &instance, window.raw_display_handle(), window.raw_window_handle(), None).unwrap() };
        
        let debug_utils = DebugUtils::new(&entry, &instance);
        let debug_messenger = debug::new_debug_messenger(&debug_utils);

        let physical_device = Self::pick_physical_device(&instance, &surface, surface_khr);
        let (device, graphics_queue, present_queue) = Self::new_logical_device_and_queues(&instance, &surface, surface_khr, physical_device);

        let (swapchain, swapchain_khr, swapchain_images, swapchain_image_format, swapchain_extent) = Self::new_swapchain_and_images(&instance, physical_device, &device, &surface, surface_khr);

        Self {
            entry,
            instance,
            event_loop,
            window,
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
            swapchain_image_format,
            swapchain_extent,
        }
    }

    fn new_swapchain_and_images(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        surface: &Surface,
        surface_khr: vk::SurfaceKHR,
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
        let extent = Self::choose_swapchain_extent(&capabilities);
        let image_count = (capabilities.min_image_count + 1).max(capabilities.max_image_count);

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

    fn choose_swapchain_extent(capabilities: &vk::SurfaceCapabilitiesKHR) -> vk::Extent2D {
        if capabilities.current_extent.width != u32::MAX {
            return capabilities.current_extent;
        }
        
        let min = capabilities.min_image_extent;
        let max = capabilities.max_image_extent;
        let width = WIDTH.min(max.width).max(min.width);
        let height = HEIGHT.min(max.height).max(min.height);
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

        debug::check_validation_layer_support(entry);

        let mut info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_name_ptrs);
            
        #[cfg(debug_assertions)] {
            debug::check_validation_layer_support(entry);
            info = info.enabled_layer_names(&layer_name_ptrs);
        }

        unsafe { entry.create_instance(&info, None).unwrap() }
    }

    fn pick_physical_device(instance: &ash::Instance, surface: &Surface, surface_khr: vk::SurfaceKHR) -> vk::PhysicalDevice {
        let device = unsafe{instance.enumerate_physical_devices()}
            .unwrap()
            .into_iter()
            .find(|device| Self::is_device_suitable(*device, surface, surface_khr, &instance))
            .unwrap();

        let props = unsafe {instance.get_physical_device_properties(device)};
        log::debug!("Selected physical device: {:?}", unsafe{CStr::from_ptr(props.device_name.as_ptr())});
        device
    }

    fn is_device_suitable(
        device: vk::PhysicalDevice, 
        surface: &Surface, 
        surface_khr: vk::SurfaceKHR, 
        instance: &ash::Instance) -> bool {
        let (graphics, present) = Self::find_queue_families(device, surface, surface_khr, &instance);
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
        physical_device: vk::PhysicalDevice) -> (ash::Device, vk::Queue, vk::Queue) {

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

            (device, graphics_queue, present_queue)
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
                panic!("placeholder")
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

    fn run(&mut self) {
        log::debug!("Running app...");
    }
}

impl Drop for VkApp {
    fn drop(&mut self) {
        log::debug!("Dropping application...");
        unsafe {
            self.device.destroy_device(None);
            self.surface.destroy_surface(self.surface_khr, None);

            #[cfg(debug_assertions)]
            self.debug_utils.destroy_debug_utils_messenger(self.debug_messenger, None);

            self.instance.destroy_instance(None);
        }
    }
}

fn main() {
    VkLogger::init(log::LevelFilter::Debug);

    let mut app = VkApp::new();
    app.run();
}
