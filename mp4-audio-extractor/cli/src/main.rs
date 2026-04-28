// ===================================================================
// main.rs — CLI 独立版本入口
// ===================================================================
//
// 本文件是一个完全独立的命令行工具，不依赖 Tauri，零外部 crate。
// 仅使用 Rust 标准库 + 系统 ffmpeg 实现 MP4 → WAV 音频提取。
//
// 【用法】
//   mp4-audio-extractor-cli.exe video.mp4
//   mp4-audio-extractor-cli.exe video1.mp4 video2.mp4  （支持批量）
//
// 【设计理念】
//   与 GUI 版本的 extract_audio() 共享相同的核心逻辑（ffmpeg 参数完全一致），
//   确保 CLI 和 GUI 的输出行为一致。

// 导入标准库模块
use std::env;                   // 读取命令行参数
use std::path::Path;            // 文件路径操作
use std::process::{Command, exit}; // 执行外部命令 + 程序退出

fn main() {
    // 收集所有命令行参数（第一个参数是程序自身的路径）
    let args: Vec<String> = env::args().collect();

    // 如果没有提供参数（只有程序名），显示帮助信息
    if args.len() < 2 {
        eprintln!("MP4 音频无损提取工具");    // eprintln! 输出到 stderr
        eprintln!("用法: mp4-audio-extractor-cli <视频文件.mp4> [视频文件2.mp4 ...]");
        eprintln!();
        eprintln!("示例:");
        eprintln!("  mp4-audio-extractor-cli video.mp4");
        eprintln!("  mp4-audio-extractor-cli video1.mp4 video2.mp4");
        exit(1);  // 非零退出码表示错误
    }

    let mut success_count = 0;
    let mut fail_count = 0;

    // 遍历除程序名外的所有文件参数
    for input_path in &args[1..] {
        match extract_audio(input_path) {
            Ok(output) => {
                println!("✅ 提取成功: {} -> {}", input_path, output);
                success_count += 1;
            }
            Err(e) => {
                eprintln!("❌ 提取失败: {} - {}", input_path, e);
                fail_count += 1;
            }
        }
        println!();
    }

    println!("处理完成: {} 个成功, {} 个失败", success_count, fail_count);
}

/// 提取 MP4 视频中的音频流并输出为 WAV 文件
///
/// 【ffmpeg 参数说明】
/// -i input     : 输入文件
/// -vn          : 丢弃视频流 (Video None)
/// -acodec pcm_s16le : 音频编码为 16-bit PCM（无压缩 WAV）
/// -y           : 覆盖已存在的同名文件
///
/// 【输出位置】输入文件同级目录，文件名为 {原文件名}_audio.wav
fn extract_audio(input_path: &str) -> Result<String, String> {
    let input = Path::new(input_path);

    // 校验1：文件存在性
    if !input.exists() {
        return Err(format!("文件不存在: {}", input_path));
    }

    // 校验2：扩展名是否为 mp4（不区分大小写）
    let extension = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if extension != "mp4" {
        return Err(format!("不支持的文件格式: .{}，仅支持 .mp4", extension));
    }

    // 构建输出路径：{父目录}/{文件名}_audio.wav
    let parent_dir = input.parent().unwrap_or(Path::new("."));
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let output_path = parent_dir.join(format!("{}_audio.wav", stem));
    let output_str = output_path.to_string_lossy().to_string();

    // 执行 ffmpeg 命令
    let status = Command::new("ffmpeg")
        .args([
            "-i", input_path,
            "-vn",                      // 丢弃视频轨
            "-acodec", "pcm_s16le",     // 编码为 16-bit PCM
            "-y",                       // 覆盖已存在文件
            &output_str,
        ])
        .status()                       // 执行并等待完成，只关心成功/失败
        .map_err(|e| format!("无法运行 ffmpeg，请确认 ffmpeg 已安装且在 PATH 中: {}", e))?;

    if status.success() {
        Ok(output_str)
    } else {
        // 失败时清理可能产生的残留文件
        let _ = std::fs::remove_file(&output_str);
        Err("ffmpeg 处理失败".to_string())
    }
}
