use bevy::prelude::*;

#[derive(Debug)]
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_startup_system(toggle_on_start)
            .add_system(toggle_mouse_capture);
    }
}

fn toggle_on_start(mut windows: ResMut<Windows>) {
    let window = windows.get_primary_mut().unwrap();
    window.set_cursor_visibility(false);
    window.set_cursor_lock_mode(true);
}

fn toggle_mouse_capture(mut windows: ResMut<Windows>, tab_input: Res<Input<KeyCode>>) {
    let window = windows.get_primary_mut().unwrap();
    let locked = window.cursor_locked();
    if tab_input.just_pressed(KeyCode::Tab) {
        window.set_cursor_visibility(locked);
        window.set_cursor_lock_mode(!locked);
    }
}
