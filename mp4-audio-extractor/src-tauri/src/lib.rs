/// ===================================================================
/// lib.rs — MP4 音频提取器 的 Rust 后端核心
/// ===================================================================
///
/// 本文件是整个 Tauri 应用的后端入口，主要包含以下内容：
///
/// 【信号处理引擎】（内部函数，不对外暴露给前端）
///   extract_pcm()            — 将任意音频统一解码为 16kHz f32 单声道 PCM
///   compute_dft()            — 手写离散傅里叶变换（DFT）
///   compute_autocorrelation() — 自相关计算（用于音高检测）
///   hann_window()            — 生成 Hann 窗系数
///   compute_spectrogram()    — 短时傅里叶变换（STFT）→ 频谱热力图数据
///   compute_pitch()          — 自相关法基频检测
///   compute_loudness()       — RMS → dB 响度计算
///   compute_waveform()       — 降采样波形（RMS+Peak 混合）
///   energy_based_vad()       — 能量阈值语音活动检测
///   align_text_to_segments() — 按 VAD 语音段等比例分配文字时序
///
/// 【Tauri 命令】（#[tauri::command]，通过 invoke_handler 暴露给前端 JS 调用）
///   pick_file()           — 弹出原生对话框选择 MP4 文件
///   extract_audio()       — 从 MP4 视频中提取 WAV 音频
///   get_audio_info()      — 通过 ffprobe 获取音频元数据
///   open_folder()         — 在资源管理器中打开文件夹
///   pick_audio_file()     — 弹出原生对话框选择音频文件（用于转换）
///   convert_audio()       — WAV/FLAC/MP3 之间格式转换
///   pick_analysis_file()  — 弹出原生对话框选择音频文件（用于分析）
///   analyze_audio()       — 完整音频分析并返回 JSON 结果
///
/// 【程序入口】
///   run()                  — 初始化 Tauri Builder，注册所有插件和命令

// ---- 外部 crate 导入 ----

use std::f64::consts::PI;          // 圆周率 π，用于 DFT 和 Hann 窗
use std::path::Path;               // 文件路径操作
use std::process::Command;         // 执行外部命令（ffmpeg / ffprobe / explorer）

use serde_json::json;              // serde_json 的 json! 宏，方便构建 JSON 对象
use tauri_plugin_dialog::DialogExt; // Tauri 文件对话框插件扩展 trait

// ===================================================================
//  信号处理引擎 — 内部函数
//  这些函数不被前端直接调用，仅供 analyze_audio() 内部编排使用
// ===================================================================

/// 将任意音频文件统一解码为 16kHz 单声道 f32 PCM 样本数组
///
/// 【两步流程】
/// 1. ffprobe 获取音频元数据（时长、原始采样率、声道数）
/// 2. ffmpeg 解码并重采样：-f f32le → f32 浮点 PCM / -ar 16000 → 重采样 / -ac 1 → 单声道
///
/// 【返回值】
/// (样本数组 Vec<f64>, 时长秒数 f64, 实际采样率 u32)
///
/// 【设计理由】
/// 统一 16kHz 单声道可以避免不同格式/采样率下的算法参数不一致问题。
/// 16kHz 对人声分析足够（Nyquist 频率 8kHz），同时降低计算量。
fn extract_pcm(input_path: &str) -> Result<(Vec<f64>, f64, u32), String> {
    // --- 第 1 步：用 ffprobe 探针获取音频元数据 ---
    let probe_out = Command::new("ffprobe")
        .args([
            "-v", "error",            // 只输出错误信息，抑制正常日志
            "-select_streams", "a:0", // 仅选择第一条音频流
            "-show_entries", "format=duration:stream=sample_rate,channels",
                                     // 需要获取：时长、采样率、声道数
            "-of", "default=noprint_wrappers=1", // 输出格式：key=value 形式，不含包装头
            input_path,
        ])
        .output()                    // 执行命令，等待完成，同时捕获 stdout 和 stderr
        .map_err(|e| format!("无法运行 ffprobe: {}", e))?;

    // 将 ffprobe 的 stdout 字节转为字符串
    let probe_str = String::from_utf8_lossy(&probe_out.stdout);

    // 设置默认值（解析失败时的回退值）
    let mut duration: f64 = 0.0;        // 默认时长

    // 逐行解析 ffprobe 输出，提取 key=value 对
    for line in probe_str.lines() {
        // strip_prefix() 是标准库方法，若字符串以指定前缀开头则返回剩余部分
        if let Some(val) = line.strip_prefix("duration=") {
            duration = val.trim().parse().unwrap_or(0.0);
        }
    }

    // --- 第 2 步：用 ffmpeg 统一解码为 f32 PCM ---
    let target_rate = 16000u32; // 统一重采样到 16kHz
    let output = Command::new("ffmpeg")
        .args([
            "-i", input_path,
            "-f", "f32le",            // 输出格式：32-bit float little-endian（原始浮点字节流）
            "-acodec", "pcm_f32le",    // 音频编码：PCM float 32-bit
            "-ar", &target_rate.to_string(), // 采样率：16000 Hz
            "-ac", "1",               // 声道数：强制单声道
            "-y",                     // 自动覆盖输出（虽然输出到 pipe 不需要，但作为习惯保留）
            "pipe:1",                 // 输出到 stdout 管道，避免创建临时文件
        ])
        .output()
        .map_err(|e| format!("无法运行 ffmpeg: {}", e))?;

    // stdout 中的原始字节转为 f64 数组
    let buf = output.stdout;
    let samples: Vec<f64> = buf
        .chunks_exact(4)    // 每个 f32 样本占 4 字节（32 bits）
        .map(|b| {
            // 将 4 字节小端序转为 f32，再转为 f64
            f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64
        })
        .collect();

    Ok((samples, duration, target_rate))
}

/// 手写离散傅里叶变换（DFT）
///
/// 【数学定义】
/// X[k] = (1/N) * sqrt( real² + imag² )
/// 其中 real  = Σ x[n]·cos(-2π·k·n/N)
///      imag  = Σ x[n]·sin(-2π·k·n/N)
///
/// 【性能说明】
/// N=1024 时每帧约 100 万次浮点运算，30 秒 16kHz 音频约 938 帧，
/// 总运算约 9.4 亿次浮点，现代 CPU 在 1-2 秒内完成。
///
/// 【返回值】
/// 仅返回前 N/2 个频点的幅度（利用 DFT 对称性，后半为镜像）
fn compute_dft(input: &[f64]) -> Vec<f64> {
    let n = input.len();              // 输入信号长度，如 1024
    let half = n / 2;                // 奈奎斯特频率以下的频点数
    let mut mags = vec![0.0; half];  // 预分配结果数组

    // 外层循环：遍历每个频点 k（0 到 N/2 - 1）
    for k in 0..half {
        let mut real = 0.0;  // 实部累加器
        let mut imag = 0.0;  // 虚部累加器

        // 内层循环：对每个时域样本 n，计算其对该频点的贡献
        for (t, &x) in input.iter().enumerate() {
            // 复指数 e^(-j·2π·k·t/N) 的相位角
            let angle = -2.0 * PI * (t as f64) * (k as f64) / (n as f64);
            real += x * angle.cos();
            imag += x * angle.sin();
        }

        // 幅度 = sqrt(real² + imag²) / N（归一化）
        mags[k] = (real * real + imag * imag).sqrt() / (n as f64);
    }

    mags
}

/// 自相关计算
///
/// 【数学定义】
/// R[k] = (1/(N-k)) · Σₙ x[n]·x[n+k]
///
/// 【用途】
/// 自相关函数的峰值位置对应于信号的基音周期。
/// 若 best_lag=100, sample_rate=16000, 则 pitch=16000/100=160Hz。
///
/// 【参数】
/// data    — 输入信号片段
/// max_lag — 最大延迟（延迟越大，可检测的最低频率越低）
fn compute_autocorrelation(data: &[f64], max_lag: usize) -> Vec<f64> {
    // 对每个延迟 lag 计算自相关系数
    (0..max_lag).map(|lag| {
        let n = data.len() - lag;  // 有效计算长度（越大的 lag 意味着数据越少）
        if n == 0 {
            return 0.0;            // 无有效数据，返回 0
        }
        // Σ x[i] · x[i + lag]
        let sum: f64 = (0..n).map(|i| data[i] * data[i + lag]).sum();
        sum / (n as f64)           // 归一化
    }).collect()
}

/// 生成 Hann 窗函数系数
///
/// 【公式】
/// w[n] = 0.5 · (1 − cos(2πn / (N−1)))
///
/// 【用途】
/// 在 STFT 分帧时对每帧加窗，减少频谱泄漏。
/// Hann 窗的旁瓣衰减约 −31.5 dB/oct，远优于矩形窗的 −6 dB/oct。
fn hann_window(size: usize) -> Vec<f64> {
    (0..size).map(|n| {
        0.5 * (1.0 - (2.0 * PI * n as f64 / (size as f64 - 1.0)).cos())
    }).collect()
}

/// 计算频谱图（短时傅里叶变换 Spectrogram）
///
/// 【原理】
/// 1. 将音频切分为重叠的帧（每帧 window_size 点，帧间间隔 hop 点）
/// 2. 每帧加 Hann 窗 → 做 DFT → 得到频域幅度
/// 3. 将频点降采样至 max_bins（128）以减少前端传输数据量
///
/// 【返回 JSON 结构】
/// {
///   "times":       [0.0, 0.032, 0.064, ...],     // 每帧的时间戳（秒）
///   "frequencies": [0.0, 62.5, 125.0, ...],      // 对应频点（Hz）
///   "magnitudes":  [[0.01, 0.03, ...], ...]      // 每帧 128 个频点的幅度值
/// }
fn compute_spectrogram(
    samples: &[f64],
    sample_rate: u32,
    window_size: usize,  // 1024
    hop: usize           // 512（帧间步长，50% 重叠）
) -> serde_json::Value {
    // 预先生成 Hann 窗（所有帧共用同一个窗）
    let window = hann_window(window_size);

    // 计算总帧数：（总样本 - 窗大小）/ 步长 + 1
    let num_windows = if samples.len() >= window_size {
        (samples.len() - window_size) / hop + 1
    } else {
        0 // 样本数不足一帧，无频谱数据
    };

    let num_bins = window_size / 2;  // DFT 后的频点数（1024/2 = 512）
    let max_bins = 128usize;          // 降采样后的频点数，降低前端渲染成本

    // 预分配数据结构
    let mut times = Vec::with_capacity(num_windows);
    let mut magnitudes: Vec<Vec<f64>> = Vec::with_capacity(num_windows);

    // 频率分辨率 = sample_rate / window_size
    let freq_step = sample_rate as f64 / window_size as f64;

    // 构建频率标签列表（前端 Y 轴用）
    let frequencies: Vec<f64> = (0..max_bins)
        .map(|i| i as f64 * freq_step * (num_bins as f64 / max_bins as f64))
        .collect();

    // 逐帧滑动窗口
    for w in 0..num_windows {
        let start = w * hop;               // 当前帧起始位置
        let end = start + window_size;     // 当前帧结束位置
        let frame: Vec<f64> = samples[start..end]
            .iter()
            .zip(&window)                  // 将信号与窗函数逐点相乘
            .map(|(&s, &win)| s * win)     // 加窗
            .collect();

        // 对加窗后的帧做 DFT，得到 512 个频点幅度
        let dft_mags = compute_dft(&frame);

        // 降采样：从 512 频点 → 128 频点（均匀抽取）
        let downsampled: Vec<f64> = (0..max_bins)
            .map(|i| {
                let idx = (i * num_bins / max_bins).min(num_bins - 1);
                dft_mags[idx]
            })
            .collect();

        magnitudes.push(downsampled);
        times.push((start as f64) / sample_rate as f64); // 帧时间戳（秒）
    }

    // 用 serde_json 的 json! 宏构建 JSON 对象
    json!({
        "times": times,
        "frequencies": frequencies,
        "magnitudes": magnitudes
    })
}

/// 自相关法基频（音高）检测
///
/// 【原理】
/// 1. 分帧（与频谱图相同的窗大小和步长）
/// 2. 每帧计算自相关函数
/// 3. 在 [min_lag, max_lag] 范围内找自相关峰值 → 峰值位置 = 基音周期
/// 4. pitch_hz = sample_rate / best_lag
///
/// 【搜索范围】
/// min_lag = sample_rate / 500 ≈ 32   → 最高可检测 500Hz
/// max_lag = sample_rate / 50  ≈ 320  → 最低可检测 50Hz
///
/// 【返回值】
/// { "times": [0.0, 0.032, ...], "values": [160.5, 0.0, ...] }
/// values 中 0.0 表示该帧为无声段（自相关峰值不足）
fn compute_pitch(
    samples: &[f64],
    sample_rate: u32,
    window_size: usize,
    hop: usize
) -> serde_json::Value {
    let num_windows = if samples.len() >= window_size {
        (samples.len() - window_size) / hop + 1
    } else {
        0
    };

    // 音频分析的音高范围：50Hz ~ 500Hz
    let min_hz = 50.0;   // 人类基频下限
    let max_hz = 500.0;  // 上限（覆盖绝大部分语音和乐器）
    let max_lag = (sample_rate as f64 / min_hz) as usize; // 最大延迟 = 320
    let min_lag = (sample_rate as f64 / max_hz) as usize; // 最小延迟 = 32

    let mut times = Vec::with_capacity(num_windows);
    let mut pitches = Vec::with_capacity(num_windows);

    for w in 0..num_windows {
        let start = w * hop;
        let end = (start + window_size).min(samples.len()); // 防止越界
        let frame = &samples[start..end];

        // 计算该帧的自相关函数
        let autocorr = compute_autocorrelation(frame, max_lag);

        // 在有效延迟范围内找自相关最大值
        let mut best_lag = 0;
        let mut best_val = 0.0;
        for lag in min_lag..max_lag.min(autocorr.len()) {
            if autocorr[lag] > best_val {
                best_val = autocorr[lag];
                best_lag = lag;
            }
        }

        // 若峰值够显著则计算音高，否则标记为 0（无声段）
        let pitch_hz = if best_lag > 0 && best_val > 0.0001 {
            sample_rate as f64 / best_lag as f64
        } else {
            0.0
        };

        times.push((start as f64) / sample_rate as f64);
        pitches.push(pitch_hz);
    }

    json!({ "times": times, "values": pitches })
}

/// RMS 响度计算（分帧 RMS → dB）
///
/// 【公式】
/// RMS  = sqrt( Σx[n]² / N )
/// dB   = 20 · log₁₀(RMS)
///
/// 【特殊处理】
/// RMS=0 时返回 -100dB（避免 log(0) 为 -∞）
fn compute_loudness(
    samples: &[f64],
    sample_rate: u32,
    window_size: usize,
    hop: usize
) -> serde_json::Value {
    let num_windows = if samples.len() >= window_size {
        (samples.len() - window_size) / hop + 1
    } else {
        0
    };

    let mut times = Vec::with_capacity(num_windows);
    let mut lufs = Vec::with_capacity(num_windows); // 注：这里不是真正的 LUFS，简化为 dB

    for w in 0..num_windows {
        let start = w * hop;
        let end = (start + window_size).min(samples.len());
        let frame = &samples[start..end];

        // 计算 RMS：所有样本平方的平均值再开根号
        let sum_sq: f64 = frame.iter().map(|&x| x * x).sum();
        let rms = (sum_sq / frame.len() as f64).sqrt();

        // RMS 转分贝
        let db = if rms > 0.0 {
            20.0 * rms.log10()  // 20·log₁₀
        } else {
            -100.0              // 静音兜底值
        };

        times.push((start as f64) / sample_rate as f64);
        lufs.push(db);
    }

    json!({ "times": times, "values": lufs })
}

/// 波形降采样（用于前端 Canvas 渲染，减少数据量）
///
/// 【算法】
/// 将全部 N 个采样点均匀切分为 target_points 个块，
/// 每块取 RMS 与 Peak 的平均值。
/// RMS 代表能量，Peak 代表瞬时峰值，两者的平均能较好反映波形的视觉轮廓。
fn compute_waveform(samples: &[f64], target_points: usize) -> Vec<f64> {
    if samples.is_empty() {
        return vec![];
    }

    // 每块包含的采样点数（向上取整）
    let chunk_size = (samples.len() as f64 / target_points as f64).ceil() as usize;

    (0..target_points).map(|i| {
        let start = i * chunk_size;
        let end = ((i + 1) * chunk_size).min(samples.len());

        // 数据不足时返回 0
        if start >= end {
            return 0.0;
        }

        let slice = &samples[start..end];

        // RMS = sqrt( Σx² / N )
        let rms: f64 = (slice.iter().map(|&x| x * x).sum::<f64>() / slice.len() as f64).sqrt();

        // Peak = max(|x|)
        let peak: f64 = slice.iter().map(|&x| x.abs()).fold(0.0, f64::max);

        // RMS 与 Peak 的平均值
        (rms + peak) / 2.0
    }).collect()
}

/// 基于能量的语音活动检测（Voice Activity Detection）
///
/// 【算法步骤】
/// 1. 分帧计算 RMS 能量
/// 2. 阈值 = max_energy × 5%
/// 3. 能量 > 阈值 → 语音段开始；能量 ≤ 阈值 → 语音段结束
/// 4. 过滤掉时长 < 50ms 的碎片段
/// 5. 合并间隔 < 100ms 的相邻段（短停顿不应拆开）
/// 6. 若整段几乎连续有语音（> 90% 时长），则返回整体段
///
/// 【返回值】
/// Vec<(start_second, end_second)> 语音段的起止时间列表
fn energy_based_vad(
    samples: &[f64],
    sample_rate: u32,
    window_size: usize,
    hop: usize
) -> Vec<(f64, f64)> {
    let num_windows = if samples.len() >= window_size {
        (samples.len() - window_size) / hop + 1
    } else {
        0
    };

    // 第一步：逐帧计算 RMS 能量
    let mut energies: Vec<f64> = vec![];
    for w in 0..num_windows {
        let start = w * hop;
        let end = (start + window_size).min(samples.len());
        let frame = &samples[start..end];
        let sum_sq: f64 = frame.iter().map(|&x| x * x).sum();
        let rms = (sum_sq / frame.len() as f64).sqrt();
        energies.push(rms);
    }

    if energies.is_empty() {
        return vec![];
    }

    // 第二步：取最大能量的 5% 作为语音/静音判定阈值
    let max_e = energies.iter().cloned().fold(0.0, f64::max);
    let threshold = max_e * 0.05;

    let mut segments: Vec<(f64, f64)> = vec![];
    let mut seg_start: Option<usize> = None; // Option: 记录当前语音段的起始帧索引

    // 第三步：逐帧遍历，根据能量阈值切分语音段
    for (i, &e) in energies.iter().enumerate() {
        if e > threshold && seg_start.is_none() {
            // 能量从阈值下面跳上来了 → 语音段开始
            seg_start = Some(i);
        } else if e <= threshold && seg_start.is_some() {
            // 能量掉到阈值以下 → 语音段结束
            let start_idx = seg_start.unwrap();
            let start_time = (start_idx * hop) as f64 / sample_rate as f64;
            let end_time = (i * hop) as f64 / sample_rate as f64;
            let dur = end_time - start_time;

            // 第四步：过滤太短的碎片（< 50ms 更可能是噪点）
            if dur >= 0.05 {
                segments.push((start_time, end_time));
            }
            seg_start = None;
        }
    }

    // 处理末尾未闭合的语音段
    if let Some(start_idx) = seg_start {
        let start_time = (start_idx * hop) as f64 / sample_rate as f64;
        let end_time = (energies.len() * hop) as f64 / sample_rate as f64;
        let dur = end_time - start_time;
        if dur >= 0.05 {
            segments.push((start_time, end_time));
        }
    }

    // 第六步：若检测到的唯一语音段覆盖了超过 90% 的总时长，
    // 说明这是一段连续讲话，直接返回整段
    if segments.len() == 1
        && segments[0].1 - segments[0].0 > samples.len() as f64 / sample_rate as f64 * 0.9
    {
        return vec![(0.0, samples.len() as f64 / sample_rate as f64)];
    }

    // 第五步：合并间隔 < 100ms 的相邻语音段
    let mut merged: Vec<(f64, f64)> = vec![];
    for &(s, e) in &segments {
        if let Some(last) = merged.last_mut() {
            // last.1 是前一段的结束时间，s 是当前段的开始时间
            if s - last.1 < 0.1 {
                last.1 = e; // 合并：把前一段的结束时间扩展到当前段
                continue;
            }
        }
        merged.push((s, e));
    }

    merged
}

/// 文字时序对齐：将用户提供的文字按语音段等比例分配时间
///
/// 【算法思路】
/// 这是一个估算方法（非 ASR 引擎）：假设说话速度在整个语音段中基本均匀，
/// 将总共有声时长按文字数量等比例分配给每个字符。
///
/// 【输入】
/// segments   — VAD 检测到的语音段时间段列表 [(start, end), ...]
/// transcript — 用户输入的对应文字内容
///
/// 【输出】
/// [{ "text": "你", "start": 0.12, "end": 0.18 }, ...]
/// 时间精确到毫秒（3 位小数）
fn align_text_to_segments(
    segments: &[(f64, f64)],
    transcript: &str,
) -> Vec<serde_json::Value> {
    // 提取非空白字符列表（中文按字/英文按字母拆分）
    let chars: Vec<char> = transcript.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if chars.is_empty() || segments.is_empty() {
        return vec![];
    }

    // 计算所有语音段的总时长
    let total_speech: f64 = segments.iter().map(|(s, e)| e - s).sum();

    let mut result = vec![];
    let mut char_idx = 0;     // 当前分配到的字符索引
    let mut accumulated = 0.0; // 已分配的累计时长

    // 遍历每个语音段
    for &(seg_start, seg_end) in segments {
        let mut t = seg_start; // 当前段内的时间游标

        // 在当前段内，尽可能多地放置字符
        while t < seg_end && char_idx < chars.len() {
            // 计算剩余未分配字符的平均分配时长
            let remaining = total_speech - accumulated;
            let remaining_chars = chars.len() - char_idx;
            if remaining_chars == 0 {
                break;
            }
            let dur = remaining / remaining_chars as f64;

            // 字符结束时间不能超出当前语音段的末尾
            let end_t = (t + dur).min(seg_end);

            result.push(json!({
                "text": chars[char_idx].to_string(),
                "start": (t * 1000.0).round() / 1000.0,     // 毫秒精度
                "end": (end_t * 1000.0).round() / 1000.0
            }));

            t = end_t;
            accumulated += dur;
            char_idx += 1;
        }
    }

    // 兜底：如果还有剩余字符未分配，追加到最后一段之后
    if char_idx < chars.len() {
        let remaining = total_speech + 0.001; // +0.001 防止除 0
        let start_t = segments.last().map(|s| s.1).unwrap_or(0.0);
        for &ch in &chars[char_idx..] {
            let end_t = start_t + remaining / chars.len() as f64;
            result.push(json!({
                "text": ch.to_string(),
                "start": (start_t * 1000.0).round() / 1000.0,
                "end": (end_t * 1000.0).round() / 1000.0
            }));
        }
    }

    result
}

// ===================================================================
//  Tauri 命令 — 对前端暴露的 API
//  每个函数通过 #[tauri::command] 宏注册，
//  前端通过 invoke("函数名", { 参数 }) 调用
// ===================================================================

/// 弹出原生文件对话框，让用户选择 MP4 视频文件
///
/// 【过滤器】仅显示 .mp4 文件
/// 【返回值】Option<String> — 用户选择了文件则返回路径，取消则返回 None
/// 【异步】async 标记让 Tauri 在后台线程执行，不阻塞 UI
#[tauri::command]
async fn pick_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file = app
        .dialog()                   // 获取对话框 API
        .file()                     // 文件选择模式
        .add_filter("MP4 视频", &["mp4"]) // 仅显示 .mp4
        .blocking_pick_file();      // 同步等待用户选择（但在 async 上下文中不阻塞 UI）

    // file 是 FilePath 类型，转为 String
    Ok(file.map(|f| f.to_string()))
}

/// 从 MP4 视频中提取音频流并输出为 WAV 文件
///
/// 【ffmpeg 参数说明】
/// -i   : 输入文件
/// -vn  : 丢弃视频流（video none）
/// -acodec pcm_s16le : 音频编码为 16-bit PCM（无压缩 WAV）
/// -y   : 自动覆盖同名输出文件
///
/// 【输出位置】源文件同级目录，文件名为 {原文件名}_audio.wav
#[tauri::command]
fn extract_audio(input_path: String) -> Result<String, String> {
    let input = Path::new(&input_path);

    // [校验1] 文件是否存在
    if !input.exists() {
        return Err(format!("文件不存在: {}", input_path));
    }

    // [校验2] 扩展名是否为 mp4
    let extension = input
        .extension()           // 获取扩展名 → Option<&OsStr>
        .and_then(|e| e.to_str()) // 转为 &str
        .unwrap_or("")         // 无扩展名则空字符串
        .to_lowercase();       // 不区分大小写

    if extension != "mp4" {
        return Err(format!("不支持的文件格式: .{}，仅支持 .mp4", extension));
    }

    // [构建输出路径] 取父目录 + 文件名_stem + _audio.wav
    let parent_dir = input.parent().unwrap_or(Path::new("."));
    let stem = input
        .file_stem()           // 取文件名（不含扩展名）
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let output_path = parent_dir.join(format!("{}_audio.wav", stem));
    let output_str = output_path.to_string_lossy().to_string();

    // [执行 ffmpeg]
    let status = Command::new("ffmpeg")
        .args([
            "-i", &input_path,
            "-vn",                        // 丢弃视频轨
            "-acodec", "pcm_s16le",        // PCM 16-bit 无压缩
            "-y",                         // 覆盖已存在文件
            &output_str,
        ])
        .status()                         // 执行命令并等待完成
        .map_err(|e| format!("无法运行 ffmpeg，请确认 ffmpeg 已安装: {}", e))?;

    if status.success() {
        Ok(output_str)
    } else {
        // 失败时清理可能产生的残留文件（如 0 字节文件）
        let _ = std::fs::remove_file(&output_str);
        Err("音频提取失败，ffmpeg 返回错误".to_string())
    }
}

/// 获取音频文件的元数据信息
///
/// 通过 ffprobe 获取第一条音频流的编码器名称、采样率、声道数、比特率
/// 返回 key=value 格式的纯文本，由前端 JS 解析
#[tauri::command]
fn get_audio_info(input_path: String) -> Result<String, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v", "error",                         // 仅输出错误
            "-select_streams", "a:0",              // 选择第一条音频流
            "-show_entries",
            "stream=codec_name,codec_long_name,sample_rate,channels,bit_rate",
                                                   // 需要获取的字段
            "-of", "default=noprint_wrappers=1",   // key=value 格式
            &input_path,
        ])
        .output()
        .map_err(|e| format!("无法运行 ffprobe: {}", e))?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| format!("解析音频信息失败: {}", e))
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!("获取音频信息失败: {}", err))
    }
}

/// 在 Windows 资源管理器中打开指定路径的文件夹
///
/// 如果 path 本身就是文件夹则直接打开，否则取其父目录。
#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    let p = Path::new(&path);

    // 智能判断：文件路径 → 取其父目录；目录路径 → 直接使用
    let dir = if p.is_dir() {
        p.to_path_buf()
    } else {
        p.parent()
            .map(|d| d.to_path_buf())
            .unwrap_or_else(|| Path::new(".").to_path_buf())
    };

    // 调用 Windows 的 explorer 命令
    Command::new("explorer")
        .arg(dir.to_string_lossy().to_string())
        .spawn() // spawn 不等待（非阻塞），打开后立即返回
        .map_err(|e| format!("无法打开文件夹: {}", e))?;

    Ok(())
}

/// 弹出原生文件对话框，选择音频文件用于格式转换
///
/// 过滤器限定 wav / flac / mp3
#[tauri::command]
async fn pick_audio_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file = app
        .dialog()
        .file()
        .add_filter("音频文件", &["wav", "flac", "mp3"])
        .blocking_pick_file();

    Ok(file.map(|f| f.to_string()))
}

/// 弹出原生文件对话框，选择音频文件用于分析
///
/// 实现与 pick_audio_file 相同，独立命名的原因是：
/// 语义清晰 + 解耦（后续可为分析专用扩展不同的过滤规则）
#[tauri::command]
async fn pick_analysis_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file = app
        .dialog()
        .file()
        .add_filter("音频文件", &["wav", "flac", "mp3"])
        .blocking_pick_file();

    Ok(file.map(|f| f.to_string()))
}

/// 音频格式转换：在 WAV / FLAC / MP3 之间自由转换
///
/// 【编码器选择策略】
/// wav  → pcm_s16le（16-bit 无压缩 PCM）
/// flac → flac（无损压缩）
/// mp3  → libmp3lame -q:a 2（LAME 编码器，VBR 高质量档位）
///
/// 【校验链】
/// 1. 文件存在
/// 2. 源格式在允许列表内
/// 3. 目标格式在允许列表内
/// 4. 源格式 ≠ 目标格式（同格式拦截）
///
/// 【输出】同级目录，原文件名 + 新扩展名
#[tauri::command]
fn convert_audio(input_path: String, target_format: String) -> Result<String, String> {
    let input = Path::new(&input_path);

    // [校验1] 文件存在
    if !input.exists() {
        return Err(format!("文件不存在: {}", input_path));
    }

    // [校验2] 源格式白名单
    let src_ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let valid_formats = ["wav", "flac", "mp3"];
    if !valid_formats.contains(&src_ext.as_str()) {
        return Err(format!("不支持的源格式: .{}，仅支持 wav/flac/mp3", src_ext));
    }

    // [校验3] 目标格式白名单
    let target = target_format.to_lowercase();
    if !valid_formats.contains(&target.as_str()) {
        return Err(format!("不支持的目标格式: .{}，仅支持 wav/flac/mp3", target));
    }

    // [校验4] 同格式拦截
    if src_ext == target {
        return Err("源格式与目标格式相同，无需转换".to_string());
    }

    // [构建输出路径]
    let parent_dir = input.parent().unwrap_or(Path::new("."));
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let output_path = parent_dir.join(format!("{}.{}", stem, target));
    let output_str = output_path.to_string_lossy().to_string();

    // [选择编码器 + 额外参数]
    let (acodec, extra_args): (&str, Vec<&str>) = match target.as_str() {
        "wav" => ("pcm_s16le", vec![]),
        "flac" => ("flac", vec![]),
        "mp3" => ("libmp3lame", vec!["-q:a", "2"]), // -q:a 2 代表 VBR 高质量
        _ => unreachable!(),                         // 前面已自名单过滤，不会到达
    };

    // [动态构造 ffmpeg 命令] 使用 Command 的链式调用
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-i").arg(&input_path).arg("-y"); // -y 覆盖已存在文件

    cmd.arg("-acodec").arg(acodec);           // 核心参数：音频编码器
    for arg in &extra_args {                   // 仅 MP3 有额外参数
        cmd.arg(arg);
    }

    cmd.arg(&output_str);

    let status = cmd
        .status()
        .map_err(|e| format!("无法运行 ffmpeg，请确认 ffmpeg 已安装: {}", e))?;

    if status.success() {
        Ok(output_str)
    } else {
        // 失败清理残留文件
        let _ = std::fs::remove_file(&output_str);
        Err("音频转换失败，ffmpeg 返回错误".to_string())
    }
}

/// 查找 whisper-cli.exe 的路径（按 exe 所在目录 → 项目根目录顺序搜索）
fn find_whisper_exe() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径: {}", e))?;
    let exe_dir = exe.parent().unwrap_or(Path::new("."));
    let candidates = [
        exe_dir.join("tools/whisper-cli.exe"),
        exe_dir.join("../../../tools/whisper-cli.exe"),
    ];
    for p in &candidates {
        if p.exists() { return Ok(p.to_string_lossy().to_string()); }
    }
    Err("未找到 whisper-cli.exe。请先运行 tools\\setup-whisper.ps1 下载安装。".to_string())
}

/// 查找 whisper 模型文件（优先 small 模型，回退 tiny）
fn find_whisper_model() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径: {}", e))?;
    let exe_dir = exe.parent().unwrap_or(Path::new("."));
    for name in &["ggml-small.bin", "ggml-tiny.bin"] {
        for base in [exe_dir.join("tools"), exe_dir.join("../../../tools")] {
            let p = base.join(name);
            if p.exists() { return Ok(p.to_string_lossy().to_string()); }
        }
    }
    Err("未找到 whisper 模型文件。请先运行 tools\\setup-whisper.ps1 下载。".to_string())
}

/// whisper.cpp 语音识别 → 词级时序标注
///
/// 两种模式：
/// - reference_text=None：纯 ASR，whisper 自动识别
/// - reference_text=Some：作为 --prompt 传入，引导识别精度大幅提升
///
/// 返回 JSON 数组: [{"text":"你","start":0.12,"end":0.18},...]
#[tauri::command]
fn transcribe_audio(input_path: String, reference_text: Option<String>) -> Result<String, String> {
    let input = Path::new(&input_path);
    if !input.exists() { return Err(format!("文件不存在: {}", input_path)); }

    let whisper_exe = find_whisper_exe()?;
    let model_path = find_whisper_model()?;

    // 将音频转为 16kHz 单声道 WAV 临时文件
    let parent = input.parent().unwrap_or(Path::new("."));
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("temp");
    let temp_wav = parent.join(format!("{}_whisper_temp.wav", stem));
    let temp_wav_str = temp_wav.to_string_lossy().to_string();

    let ff_status = Command::new("ffmpeg")
        .args(["-i", &input_path, "-ar", "16000", "-ac", "1", "-y", &temp_wav_str])
        .status()
        .map_err(|e| format!("无法运行 ffmpeg: {}", e))?;
    if !ff_status.success() { let _ = std::fs::remove_file(&temp_wav_str); return Err("音频预处理失败".to_string()); }

    // 构造 whisper 命令
    let mut cmd = Command::new(&whisper_exe);
    cmd.arg("-m").arg(&model_path);
    cmd.arg("-f").arg(&temp_wav_str);
    cmd.arg("-l").arg("zh");
    cmd.arg("--output-json");
    cmd.arg("--word-thold").arg("0.01");

    if let Some(ref text) = reference_text {
        if !text.trim().is_empty() { cmd.arg("--prompt").arg(text.trim()); }
    }

    let output = cmd.output()
        .map_err(|e| format!("无法运行 whisper: {}。请先运行 tools\\setup-whisper.ps1", e))?;
    let _ = std::fs::remove_file(&temp_wav_str);

    if !output.status.success() {
        return Err(format!("whisper 识别失败: {}", String::from_utf8_lossy(&output.stderr)));
    }

    // 解析 JSON 提取词级时间戳
    let stdout = String::from_utf8_lossy(&output.stdout);
    let root: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("解析 whisper 输出失败: {}", e))?;

    let mut words = vec![];
    if let Some(trans) = root["transcription"].as_array() {
        for seg in trans {
            if let Some(tokens) = seg["tokens"].as_array() {
                for token in tokens {
                    let text = token["text"].as_str().unwrap_or("").to_string();
                    if text.is_empty() || text == " " { continue; }
                    let s = token["offsets"]["from"].as_u64().unwrap_or(0) as f64 / 1000.0;
                    let e = token["offsets"]["to"].as_u64().unwrap_or(0) as f64 / 1000.0;
                    if e <= s { continue; }
                    words.push(json!({"text":text,"start":(s*1000.0).round()/1000.0,"end":(e*1000.0).round()/1000.0}));
                }
            }
        }
    }
    serde_json::to_string(&words).map_err(|e| format!("序列化失败: {}", e))
}

/// 音频分析主函数：信号处理 + whisper ASR
///
/// 始终调用 whisper 进行词级 ASR。有参考文本时作为 prompt 传入引导识别。
/// whisper 不可用时 text_alignment 返回 {"error":"..."}，信号分析部分不受影响。
#[tauri::command]
fn analyze_audio(input_path: String, transcript: Option<String>) -> Result<String, String> {
    let input = Path::new(&input_path);
    if !input.exists() { return Err(format!("文件不存在: {}", input_path)); }

    let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !["wav","flac","mp3"].contains(&ext.as_str()) {
        return Err(format!("不支持的格式: .{}，仅支持 wav/flac/mp3", ext));
    }

    let (samples, duration, sample_rate) = extract_pcm(&input_path)?;
    if samples.is_empty() { return Err("音频数据为空".to_string()); }

    let window_size = 1024usize;
    let hop = 512usize;

    let waveform = compute_waveform(&samples, 1200);
    let spectrogram = compute_spectrogram(&samples, sample_rate, window_size, hop);
    let pitch = compute_pitch(&samples, sample_rate, window_size, hop);
    let loudness = compute_loudness(&samples, sample_rate, window_size, hop);

    let text_alignment = match transcribe_audio(input_path.to_string_lossy().to_string(), transcript) {
        Ok(json_str) => serde_json::from_str::<serde_json::Value>(&json_str)
            .unwrap_or_else(|_| json!({"error": "无法解析 whisper 结果"}))
            .clone(),
        Err(e) => json!({"error": e}),
    };

    let result = json!({
        "duration":duration,"sample_rate":sample_rate,"channels":1,
        "waveform":waveform,"spectrogram":spectrogram,
        "pitch":pitch,"loudness":loudness,
        "text_alignment":text_alignment
    });
    serde_json::to_string(&result).map_err(|e| format!("序列化失败: {}", e))
}

// ===================================================================
//  程序入口 — Tauri Builder 组装
// ===================================================================

/// Tauri 应用启动入口
///
/// 1. 初始化 Tauri Builder
/// 2. 注册 dialog 插件（原生文件对话框）
/// 3. 注册所有 #[tauri::command] 函数（前后端桥接）
/// 4. 加载 tauri.conf.json 配置并运行
///
/// #[cfg_attr(mobile, tauri::mobile_entry_point)]:
///   条件编译属性 — 在移动平台编译时使用 mobile_entry_point 宏
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 注册 dialog 插件，使 .dialog() 调用可用
        .plugin(tauri_plugin_dialog::init())
        // 注册所有命令（前端 invole 的名字与函数名对应）
        .invoke_handler(tauri::generate_handler![
            pick_file,          // MP4 视频选择
            extract_audio,      // 视频→WAV 提取
            get_audio_info,     // ffprobe 元数据
            open_folder,        // 打开资源管理器
            pick_audio_file,    // 音频文件选择（转换用）
            convert_audio,      // 格式转换
            pick_analysis_file, // 音频文件选择（分析用）
            analyze_audio,       // 音频信号分析
            transcribe_audio     // whisper ASR 语音识别
        ])
        // 加载 tauri.conf.json 配置（窗口大小、安全策略等）
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
