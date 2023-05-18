const KEY_CODE_COUNT: usize = 40;

pub struct InputState {
    pub keys_pressed: [bool; 40],
    pub previous_keys_pressed: [bool; 40],
    pub delta_mouse_pos: [f32; 2],
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys_pressed: [false; KEY_CODE_COUNT],
            previous_keys_pressed: [false; KEY_CODE_COUNT],
            delta_mouse_pos: [0.0, 0.0],
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
}

