use std::{ffi::CStr, rc::Rc};

use ash::{vk, extensions::khr::{Surface, Swapchain}};

use crate::debug;

pub fn pick_physical_device(
    instance: &ash::Instance, 
    surface: &Surface, 
    surface_khr: vk::SurfaceKHR) -> vk::PhysicalDevice {
    let physical_device = unsafe{instance.enumerate_physical_devices()}
        .unwrap()
        .into_iter()
        .find(|device| is_physical_device_suitable(*device, surface, surface_khr, &instance))
        .unwrap();

    let props = unsafe {instance.get_physical_device_properties(physical_device)};
    log::debug!("Selected physical device: {:?}", unsafe{CStr::from_ptr(props.device_name.as_ptr())});
    physical_device
}

pub fn is_physical_device_suitable(
    physical_device: vk::PhysicalDevice, 
    surface: &Surface, 
    surface_khr: vk::SurfaceKHR, 
    instance: &ash::Instance) -> bool {
    let (graphics, present) = find_queue_families(physical_device, surface, surface_khr, &instance);
    graphics.is_some() && present.is_some()
}

pub fn find_queue_families(
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

pub fn new_logical_device_and_queues(
    instance: &ash::Instance, 
    surface: &Surface,
    surface_khr: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice
) -> (Rc<ash::Device>, vk::Queue, vk::Queue) {

    let (graphics_family_index, present_family_index) = find_queue_families(physical_device, surface, surface_khr, instance);
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

    let (_, layer_name_ptrs) = &debug::get_layer_names_and_ptrs();

    let physical_device_features = vk::PhysicalDeviceFeatures::builder();
    let (_, device_extension_name_ptrs) = &get_device_extension_names_and_ptrs();

    check_device_extension_support(&instance, physical_device);

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

pub fn check_device_extension_support(instance: &ash::Instance, physical_device: vk::PhysicalDevice) {
    let (required_extensions, _) = &get_device_extension_names_and_ptrs();

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

pub fn get_device_extension_names_and_ptrs() -> (Vec<&'static CStr>, Vec<*const i8>) {
    let c_device_extension_names = vec![Swapchain::name()];
    let device_extension_name_ptrs = c_device_extension_names.iter()
        .map(|name| name.as_ptr())
        .collect::<Vec<_>>();

    (c_device_extension_names, device_extension_name_ptrs)
}