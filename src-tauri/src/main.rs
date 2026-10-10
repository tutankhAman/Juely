// Hide the console window on Windows (GUI subsystem, not console).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pluely_lib::run()
}