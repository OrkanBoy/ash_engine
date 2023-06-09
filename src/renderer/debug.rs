use std::ffi::{c_void, CStr, CString};

use ash::{
    extensions::ext::DebugUtils,
    vk::{self, DebugUtilsMessengerEXT},
};

const LAYER_NAMES: [&str; 1] = ["VK_LAYER_KHRONOS_validation"];

unsafe extern "system" fn vulkan_debug_callback(
    flag: vk::DebugUtilsMessageSeverityFlagsEXT,
    typ: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut c_void,
) -> vk::Bool32 {
    type Flag = vk::DebugUtilsMessageSeverityFlagsEXT;

    let msg = format!(
        "(Validation Layer): {:?} - {:?}",
        typ,
        CStr::from_ptr((*p_callback_data).p_message)
    );
    match flag {
        Flag::VERBOSE => log::debug!("{msg}"),
        Flag::INFO => log::info!("{msg}"),
        Flag::WARNING => log::warn!("{msg}"),
        _ => log::error!("{msg}"),
    }
    vk::FALSE
}

pub fn check_validation_layer_support(entry: &ash::Entry) {
    for required in LAYER_NAMES.iter() {
        let found = entry
            .enumerate_instance_layer_properties()
            .unwrap()
            .iter()
            .any(|layer| {
                let name = unsafe { CStr::from_ptr(layer.layer_name.as_ptr()) };
                let name = name.to_str().unwrap();
                required == &name
            });

        if !found {
            panic!("Validation layer not supported: {}", required);
        }
    }
}

pub fn new_messenger(debug_entry: &DebugUtils) -> DebugUtilsMessengerEXT {
    let create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(vulkan_debug_callback));

    unsafe {
        debug_entry
            .create_debug_utils_messenger(&create_info, None)
            .unwrap()
    }
}

//Return CString to avoid dangling ptrs
pub fn get_layer_names_and_ptrs() -> (Vec<CString>, Vec<*const i8>) {
    let layer_names = LAYER_NAMES
        .iter()
        .map(|name| CString::new(*name).expect("Failed to build CString"))
        .collect::<Vec<_>>();
    let layer_names_ptrs = layer_names
        .iter()
        .map(|name| name.as_ptr())
        .collect::<Vec<_>>();
    (layer_names, layer_names_ptrs)
}
