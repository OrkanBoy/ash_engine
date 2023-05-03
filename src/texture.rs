use std::rc::Rc;

use ash::vk;

pub struct Texture {
    device: Rc<ash::Device>,
    pub image_handle: vk::Image,
    pub memory: vk::DeviceMemory,
    pub size: vk::DeviceSize,
}

impl Texture {
    /*pub fn new(path: &str) -> Self {
        let image = image::open(path).expect("Unable to open image");

        Self {

        }

    }*/
}