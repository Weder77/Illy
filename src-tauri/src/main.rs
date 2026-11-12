// Pas de console en version finale sous Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    illy_app_lib::run()
}
