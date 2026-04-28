# MP4 Audio Extractor

一个基于 Tauri 的 MP4 视频音频提取工具，支持无损提取音频流并转换为 WAV 格式。

## Features

- 🎵 **MP4 音频提取** - 无损提取 MP4 视频中的音频流
- 🔄 **音频格式转换** - 支持 WAV/FLAC/MP3 格式互转
- 📊 **音频分析** - 波形、频谱、音高、响度可视化
- 🎙️ **语音识别** - 集成 whisper.cpp 进行语音转文字

## Requirements

- Rust 1.95.0+
- Node.js 24.x+
- ffmpeg 8.0.1+

## Getting Started

```bash
# Install dependencies
npm install

# Run development mode
npm run dev

# Build production
npm run build
```

## Usage

1. 运行应用
2. 点击"选择文件"选择 MP4 视频
3. 点击"提取音频"
4. 提取的 WAV 文件将保存在原视频同一目录

## License

MIT
