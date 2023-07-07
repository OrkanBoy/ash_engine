#![feature(generic_const_exprs)]

pub mod renderer;
pub mod math;
pub mod input;
pub mod components;
pub mod memory;
pub mod data_structures;
pub mod camera;

use winit::dpi::PhysicalPosition;
use winit::event::{DeviceEvent, WindowEvent, ElementState};
use winit::window::CursorGrabMode;
use winit::{event::VirtualKeyCode, event_loop::EventLoop, window::WindowBuilder, dpi::PhysicalSize};
use ash::vk::Extent2D;

use crate::data_structures::darray::Darray;
use crate::renderer::{VkApp, START_WINDOW_HEIGHT, START_WINDOW_WIDTH};

fn init_game(app: &mut VkApp) {
}

fn update_game(app: &mut VkApp, dt: f32) {
}

fn handle_input(app: &mut VkApp) {
    if !app.input_state.is_key_pressed(VirtualKeyCode::Escape) &&
        app.input_state.was_key_pressed(VirtualKeyCode::Escape) {
        app.in_game = !app.in_game;
        app.window.set_cursor_visible(!app.in_game);
        //NOTE: CursorGrabMode::Locked Not implemented by winit
        app.window.set_cursor_grab(
            if app.in_game {
                CursorGrabMode::Confined
            } else {
                CursorGrabMode::None
            }
        ).unwrap();

        if !app.in_game {
            app.window.set_cursor_position(
                PhysicalPosition {
                    x: app.swapchain_extent.width / 2,
                    y: app.swapchain_extent.height / 2,
                }
            ).unwrap();
        }
    }
}

fn handle_in_game_input(app: &mut VkApp, dt: f32) {
    if !app.in_game {
        return;
    }

    let camera = &mut app.camera;

    let dtranslation = camera.translation_speed * dt;
    let drotation = camera.rotation_speed * dt;

    let dc = dtranslation * camera.z_x_angle.cos();
    let ds = dtranslation * camera.z_x_angle.sin();
    if app.input_state.is_key_pressed(VirtualKeyCode::W) {
        camera.translation.z += dc;
        camera.translation.x += ds;
    } 
    if app.input_state.is_key_pressed(VirtualKeyCode::S) {
        camera.translation.z -= dc;
        camera.translation.x -= ds;
    }
    if app.input_state.is_key_pressed(VirtualKeyCode::D) {
        camera.translation.z -= ds;
        camera.translation.x += dc;
    } 
    if app.input_state.is_key_pressed(VirtualKeyCode::A) {
        camera.translation.z += ds;
        camera.translation.x -= dc;
    }

    camera.z_x_angle  += drotation * app.input_state.delta_mouse_pos[0];
    camera.y_xz_angle += drotation * app.input_state.delta_mouse_pos[1];
}

fn main() {
    //app init
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Ash Window")
        .with_inner_size(PhysicalSize {
            width: START_WINDOW_WIDTH, 
            height: START_WINDOW_HEIGHT,
        })
        .build(&event_loop)
        .unwrap();
    let mut app = VkApp::new(window);
    init_game(&mut app);
    
    //running app
    let mut dirty_swapchain = false;
    let mut start_frame_time = 0.0;
    let mut end_frame_time = app.start_instant.elapsed().as_secs_f32();

    use winit::{event_loop::ControlFlow, event::Event};
    event_loop.run(move |system_event, _, control_flow| {
        match system_event {
            Event::MainEventsCleared => {
                //timing
                start_frame_time = end_frame_time;
                end_frame_time = app.start_instant.elapsed().as_secs_f32();
                let dt = end_frame_time - start_frame_time;

                handle_input(&mut app);
                handle_in_game_input(&mut app, dt);
                update_game(&mut app, dt);

                app.input_state.previous_keys_pressed_bitmask = app.input_state.keys_pressed_bitmask;
                app.input_state.delta_mouse_pos = [0.0, 0.0];

                if dirty_swapchain {
                    if app.swapchain_extent.width != 0 && app.swapchain_extent.height != 0 {
                        app.renew_swapchain();
                    } else {
                        return;
                    }
                }
                dirty_swapchain = app.draw_frame();

                let fps = (1.0 / dt) as u32;
                app.window.set_title(&("fps: ".to_owned() + &fps.to_string()));
            }
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta, .. } => {
                    app.input_state.delta_mouse_pos = [delta.0 as f32, delta.1 as f32];
                }
                _ => {}
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(v_keycode) = input.virtual_keycode {
                        app.input_state.set_key_pressed(v_keycode, input.state == ElementState::Pressed);
                    }
                }
                WindowEvent::Resized(PhysicalSize {width, height}) => {
                    dirty_swapchain = true;
                    app.swapchain_extent = Extent2D {width, height};
                    app.camera.aspect_ratio = width as f32 / height as f32;
                }
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => {}
            } 
            _ => {}
        }
    })
}