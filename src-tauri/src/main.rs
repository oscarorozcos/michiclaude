// Evita que se abra una consola junto a la app en Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    claude_code_meter_lib::run();
}
