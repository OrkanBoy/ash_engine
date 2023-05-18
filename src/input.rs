const KEY_CODE_COUNT: usize = 40;

pub struct InputState {
    pub keys_pressed: [bool; 40],
    pub previous_keys_pressed: [bool; 40], 
    pub mouse_pos: [u32; 2],
    pub previous_mouse_pos: [u32; 2],
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys_pressed: [false; KEY_CODE_COUNT],
            previous_keys_pressed: [false; KEY_CODE_COUNT],
            mouse_pos: [0, 0],
            previous_mouse_pos: [0, 0],
        }
    }

    #[inline]
    pub fn is_key_pressed(&mut self, key_code: winit::event::VirtualKeyCode) -> bool {
        let key_code = key_code as usize;
        assert!(key_code < KEY_CODE_COUNT, "Not supported keycodes above value {}", KEY_CODE_COUNT);
        self.keys_pressed[key_code]
    }

    #[inline]
    pub fn was_key_pressed(&mut self, key_code: winit::event::VirtualKeyCode) -> bool {
        let key_code = key_code as usize;
        assert!(key_code < KEY_CODE_COUNT, "Not supported keycodes above value {}", KEY_CODE_COUNT);
        self.previous_keys_pressed[key_code]
    }

    #[inline]
    pub fn set_key_pressed(&mut self, key_code: winit::event::VirtualKeyCode, pressed: bool) {
        let key_code = key_code as usize;
        assert!(key_code < KEY_CODE_COUNT, "Not supported keycodes above value {}", KEY_CODE_COUNT);
        self.keys_pressed[key_code] = pressed;
    } 

    #[inline]
    pub fn calc_delta_mouse_as_f32(&self) -> [f32; 2] {
        [
            self.mouse_pos[0] as f32 - self.previous_mouse_pos[0] as f32,
            self.mouse_pos[1] as f32 - self.previous_mouse_pos[1] as f32,
        ]
    }
}

