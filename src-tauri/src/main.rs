// 禁止在 release 下显示控制台窗口（Windows 惯例）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    easyide_lib::run()
}
