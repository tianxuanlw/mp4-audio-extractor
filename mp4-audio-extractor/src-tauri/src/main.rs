// ===================================================================
// main.rs — Tauri 应用入口点（二进制 crate）
// ===================================================================
//
// 本文件是编译产物的入口（src-tauri/Cargo.toml 中通过 [[bin]] 定义）。
// 功能极其简单：禁用在发布模式下弹出控制台窗口，然后调用
// mp4_audio_extractor_lib::run() 启动 Tauri 应用。
//
// cfg_attr(not(debug_assertions), ...) 的含义：
//   仅在非 Debug 构建（即 release 模式）时才会生效。
//   windows_subsystem = "windows" 会隐藏 Windows 控制台窗口，
//   使应用看起来像原生 GUI 程序而非命令行程序。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    mp4_audio_extractor_lib::run()
}
