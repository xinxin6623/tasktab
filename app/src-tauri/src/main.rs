// 阻止 Windows release 下弹出控制台窗口（macOS 无影响，保留脚手架默认）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    taskboard_lib::run()
}
