use bevy::{prelude::*, window::CursorGrabMode};
use leafwing_input_manager::{prelude::*, Actionlike};

#[derive(Debug)]
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(toggle_on_start)
            .add_system(toggle_mouse_capture)
            .add_plugin(InputManagerPlugin::<Actions>::default());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, Actionlike)]
pub enum Actions {
    Forward,
    Backwards,
    StrafeLeft,
    StrafeRight,
    Aim,
    Sprint,
    ShootA,
    ShootB,
    ShootCube,
    Jump,
    Grab,
}

pub fn default_input_map() -> InputMap<Actions> {
    let mut input_map = InputMap::new([
        (KeyCode::W, Actions::Forward),
        (KeyCode::S, Actions::Backwards),
        (KeyCode::A, Actions::StrafeLeft),
        (KeyCode::D, Actions::StrafeRight),
        (KeyCode::F, Actions::Grab),
        (KeyCode::Q, Actions::ShootCube),
        (KeyCode::LShift, Actions::Sprint),
        (KeyCode::Space, Actions::Jump),
    ]);
    input_map.insert(DualAxis::mouse_motion(), Actions::Aim);
    input_map.insert(MouseButton::Left, Actions::ShootA);
    input_map.insert(MouseButton::Right, Actions::ShootB);

    input_map
}

fn toggle_on_start(mut windows: ResMut<Windows>) {
    let window = windows.get_primary_mut().unwrap();
    window.set_cursor_visibility(false);
    window.set_cursor_grab_mode(CursorGrabMode::Confined);
}

fn toggle_mouse_capture(mut windows: ResMut<Windows>, tab_input: Res<Input<KeyCode>>) {
    let window = windows.get_primary_mut().unwrap();
    if tab_input.just_pressed(KeyCode::Tab) {
        if window.cursor_visible() {
            window.set_cursor_visibility(false);
            window.set_cursor_grab_mode(CursorGrabMode::Confined);
        } else {
            window.set_cursor_visibility(true);
            window.set_cursor_grab_mode(CursorGrabMode::None);
        }
    }
}
