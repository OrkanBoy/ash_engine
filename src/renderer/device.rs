use std::{ffi::CStr, rc::Rc};

use ash::{
    extensions::khr::{Surface, Swapchain},
    vk,
};

pub fn get_physical_device_and_queue_family_indices(
    instance: &ash::Instance,
    surface: &Surface,
    surface_khr: vk::SurfaceKHR,
) -> (vk::PhysicalDevice, u32, u32, u32) {
    let physical_devices = unsafe { instance.enumerate_physical_devices() }.unwrap();

    let mut physical_device = physical_devices[0];
    let mut extension_support = check_device_extension_support(instance, physical_device);
    let features = unsafe { instance.get_physical_device_features(physical_device) };
    let mut feature_support = features.sampler_anisotropy == vk::TRUE;
    let (mut graphics, mut present, mut transfer) =
        find_queue_family_indices(physical_device, surface, surface_khr, instance);

    let mut i = 1;
    while i < physical_devices.len()
        && (graphics.is_none()
            || present.is_none()
            || transfer.is_none()
            || !extension_support
            || !feature_support)
    {
        physical_device = physical_devices[i];
        extension_support = check_device_extension_support(instance, physical_device);

        (graphics, present, transfer) =
            find_queue_family_indices(physical_device, surface, surface_khr, &instance);
        let features = unsafe { instance.get_physical_device_features(physical_device) };
        feature_support = features.sampler_anisotropy == vk::TRUE;

        i += 1;
    }

    let props = unsafe { instance.get_physical_device_properties(physical_device) };
    log::debug!("Selected physical device: {:?}", unsafe {
        CStr::from_ptr(props.device_name.as_ptr())
    });

    (
        physical_device,
        graphics.unwrap(),
        present.unwrap(),
        transfer.unwrap(),
    )
}

fn find_queue_family_indices(
    physical_device: vk::PhysicalDevice,
    surface: &Surface,
    surface_khr: vk::SurfaceKHR,
    instance: &ash::Instance,
) -> (Option<u32>, Option<u32>, Option<u32>) {
    let props = unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    // family indices
    let mut graphics = None;
    let mut present = None;
    let mut transfer = None;

    for (index, family_props) in props.iter().filter(|p| p.queue_count > 0).enumerate() {
        let index = index as u32;

        if family_props.queue_flags.contains(vk::QueueFlags::GRAPHICS) && graphics.is_none() {
            graphics = Some(index);
        }

        let present_support = unsafe {
            surface.get_physical_device_surface_support(physical_device, index, surface_khr)
        }
        .unwrap();
        if present_support
            && (present.is_none() || (graphics.is_some() && graphics.unwrap() == present.unwrap()))
        {
            present = Some(index);
        }

        if family_props.queue_flags.contains(vk::QueueFlags::TRANSFER)
            && (transfer.is_none()
                || (graphics.is_some() && graphics.unwrap() == transfer.unwrap())
                || (present.is_some() && present.unwrap() == transfer.unwrap()))
        {
            transfer = Some(index)
        }
    }

    (graphics, present, transfer)
}

pub fn new_logical_device_and_queues(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    graphics_family_index: u32,
    present_family_index: u32,
    transfer_family_index: u32,
) -> (Rc<ash::Device>, vk::Queue, vk::Queue, vk::Queue) {
    let queue_priorities = [1.0];

    let queue_infos = {
        let mut indices = vec![
            graphics_family_index,
            present_family_index,
            transfer_family_index,
        ];
        indices.dedup();

        indices
            .iter()
            .map(|&index| {
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(index)
                    .queue_priorities(&queue_priorities)
                    .build()
            })
            .collect::<Vec<_>>()
    };

    let (_, layer_name_ptrs) = &super::debug::get_layer_names_and_ptrs();

    let physical_device_features = vk::PhysicalDeviceFeatures::builder()
        .fill_mode_non_solid(true)
        .sampler_anisotropy(true);

    let (_, device_extension_name_ptrs) = &get_device_extension_names_and_ptrs();

    let mut info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_infos)
        .enabled_features(&physical_device_features)
        .enabled_extension_names(&device_extension_name_ptrs);

    #[cfg(debug_assertions)]
    {
        info = info.enabled_layer_names(&layer_name_ptrs);
    }

    unsafe {
        let device = instance
            .create_device(physical_device, &info, None)
            .unwrap();
        let graphics_queue = device.get_device_queue(graphics_family_index, 0);
        let present_queue = device.get_device_queue(present_family_index, 0);
        let transfer_queue = device.get_device_queue(transfer_family_index, 0);

        (
            Rc::from(device),
            graphics_queue,
            present_queue,
            transfer_queue,
        )
    }
}

pub fn check_device_extension_support(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> bool {
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
            return false;
        };
    }
    true
}

pub fn get_device_extension_names_and_ptrs() -> (Vec<&'static CStr>, Vec<*const i8>) {
    let c_device_extension_names = vec![Swapchain::name()];
    let device_extension_name_ptrs = c_device_extension_names
        .iter()
        .map(|name| name.as_ptr())
        .collect::<Vec<_>>();

    (c_device_extension_names, device_extension_name_ptrs)
}

pub fn find_mem_type_index(
    supported_types_mask: u32,
    required_props: vk::MemoryPropertyFlags,
    physical_device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
) -> u32 {
    for i in 0..physical_device_memory_properties.memory_type_count {
        if supported_types_mask & (1 << i) != 0
            && physical_device_memory_properties.memory_types[i as usize]
                .property_flags
                .contains(required_props)
        {
            return i;
        }
    }
    panic!("Could not find suitable memory type");
}

pub fn find_depth_format(instance: &ash::Instance, device: vk::PhysicalDevice) -> vk::Format {
    const CANDIDATES: [vk::Format; 3] = [
        vk::Format::D32_SFLOAT,
        vk::Format::D32_SFLOAT_S8_UINT,
        vk::Format::D24_UNORM_S8_UINT,
    ];

    find_supported_format(
        instance,
        device,
        &CANDIDATES,
        vk::ImageTiling::OPTIMAL,
        vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
    )
    .expect("Failed to find a supported depth format")
}

/// Find the first compatible format from `candidates`.
pub fn find_supported_format(
    instance: &ash::Instance,
    device: vk::PhysicalDevice,
    candidates: &[vk::Format],
    tiling: vk::ImageTiling,
    features: vk::FormatFeatureFlags,
) -> Option<vk::Format> {
    match tiling {
        vk::ImageTiling::LINEAR => candidates.iter().map(|&f| f).find(|&candidate| {
            let props =
                unsafe { instance.get_physical_device_format_properties(device, candidate) };

            props.linear_tiling_features.contains(features)
        }),
        vk::ImageTiling::OPTIMAL => candidates.iter().map(|&f| f).find(|&candidate| {
            let props =
                unsafe { instance.get_physical_device_format_properties(device, candidate) };

            props.optimal_tiling_features.contains(features)
        }),
        _ => None,
    }
}
