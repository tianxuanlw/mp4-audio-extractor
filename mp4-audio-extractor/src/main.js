/**
 * ===================================================================
 * main.js — MP4 音频提取器 前端交互逻辑
 * ===================================================================
 *
 * 本文件是 Tauri 应用的前端核心，负责三大功能模块的 UI 交互：
 *
 * 【1. 视频提取】 选择 MP4 → 查看音频信息 → 一键提取为 WAV
 * 【2. 音频转换】 选择音频 → 选择目标格式（三选一 radio）→ 格式转换
 * 【3. 音频分析】 导航至分析页 → 拖入/选择音频 → 可选填文字 → 分析 → Canvas 图表渲染
 *
 * 架构说明：
 * - 所有与 Rust 后端的通信通过 window.__TAURI__.core.invoke("命令名", {参数}) 进行
 * - 页面采用单页双视图设计：mainView（主页三卡片）↔ analysisView（分析详情页）
 * - Canvas 图表全部手写渲染，不依赖任何第三方图表库
 */

// ---- 全局 ----

// 从 Tauri 运行时获取 invoke 函数（前后端桥接的核心 API）
// window.__TAURI__ 是 Tauri 注入到 WebView 中的全局对象
const { invoke } = window.__TAURI__?.core ?? {};

// 三个功能模块各自独立的状态变量（互不干扰）
let selectedFilePath = null;      // 视频提取：当前选中的 MP4 文件路径
let selectedConvertPath = null;   // 音频转换：当前选中的音频文件路径
let selectedAnalysisPath = null;  // 音频分析：当前选中的音频文件路径

/* ===================================================================
   页面导航：主页 ↔ 分析详情页
   =================================================================== */

const mainView = document.getElementById("mainView");           // 主页面容器
const analysisView = document.getElementById("analysisView");    // 分析详情页容器
const container = document.querySelector(".container");         // 最外层容器（控制 max-width）
const navAnalysis = document.getElementById("navAnalysis");     // "音频分析" 导航卡片
const btnBack = document.getElementById("btnBack");              // 分析页的 "← 返回" 按钮

// 点击主页底部的 "🔬 音频分析" 导航卡片 → 切换到分析页
navAnalysis.addEventListener("click", () => {
  mainView.classList.add("hidden");         // 隐藏主页
  analysisView.classList.remove("hidden");  // 显示分析页
  container.classList.add("wide");          // 容器宽度从 520px 扩大到 820px（分析页需要更宽的空间）
});

// 点击分析页顶部的 "← 返回" 按钮 → 回到主页
btnBack.addEventListener("click", () => {
  analysisView.classList.add("hidden");     // 隐藏分析页
  mainView.classList.remove("hidden");      // 显示主页
  container.classList.remove("wide");       // 恢复窄容器宽度
  resetAnalysis();                          // 清空分析页的所有状态
});

/* ===================================================================
   模块一：视频提取
   功能：选择 MP4 文件 → 显示音频信息 → 提取 WAV 音频
   =================================================================== */

// ---- DOM 元素引用 ----
const btnSelect = document.getElementById("btnSelect");             // "选择文件" 按钮
const btnExtract = document.getElementById("btnExtract");           // "提取音频" 主按钮
const btnClear = document.getElementById("btnClear");               // 文件名旁的 ✕ 清除按钮
const btnOpenFolder = document.getElementById("btnOpenFolder");     // "打开所在文件夹" 按钮（提取成功后显示）
const btnExtractText = document.getElementById("btnExtractText");   // 提取按钮内的文字（用于切换文案）
const spinner = document.getElementById("spinner");                 // 加载旋转动画

// 文件选择区域的状态切换元素
const filePlaceholder = document.getElementById("filePlaceholder"); // 未选择时的占位区（虚线框）
const fileSelected = document.getElementById("fileSelected");       // 已选择时的文件名显示条
const fileName = document.getElementById("fileName");               // 文件名文本

// 音频信息面板
const audioInfo = document.getElementById("audioInfo");  // 音频信息容器
const infoGrid = document.getElementById("infoGrid");    // 信息网格

// 操作状态提示
const statusArea = document.getElementById("statusArea");  // 状态提示容器
const statusIcon = document.getElementById("statusIcon");  // ✅ / ❌ 图标
const statusText = document.getElementById("statusText");  // 状态文字

// ---- 事件绑定 ----

// 点击 "选择文件" → 调用 Rust 后端弹出原生 MP4 文件对话框
btnSelect.addEventListener("click", async () => {
  try {
    const path = await invoke("pick_file");  // 调用 Rust 的 pick_file 命令
    if (path) await setFile(path);            // 用户选择了文件（非取消）
  } catch (e) {
    showStatus("error", "打开文件对话框失败: " + e);
  }
});

// 点击 ✕ → 清除选择，恢复初始状态
btnClear.addEventListener("click", () => resetFile());

// 点击 "提取音频" → 调用 Rust 后端执行 ffmpeg 提取
btnExtract.addEventListener("click", async () => {
  if (!selectedFilePath) return;  // 防御：未选文件时不应触发

  setLoading(true);   // 进入加载态：禁用按钮、显示 spinner
  hideStatus();       // 清除上次的状态提示

  try {
    const outputPath = await invoke("extract_audio", { inputPath: selectedFilePath });
    showStatus("success", "音频提取成功！已保存至: " + outputPath);
    btnOpenFolder.classList.remove("hidden");             // 显示 "打开文件夹" 按钮
    btnOpenFolder.dataset.outputPath = outputPath;         // 存储输出路径以便后续打开
  } catch (e) {
    showStatus("error", "提取失败: " + e);
  } finally {
    setLoading(false);  // 无论成功或失败，都恢复按钮状态
  }
});

// 点击 "打开所在文件夹" → 在资源管理器中定位到输出文件
btnOpenFolder.addEventListener("click", () => {
  const path = btnOpenFolder.dataset.outputPath;
  if (path) invoke("open_folder", { path }).catch(() => {});
});

// ---- 内部函数 ----

// 选中文件后的处理：更新文件名显示 → 加载音频信息 → 启用提取按钮
async function setFile(path) {
  selectedFilePath = path;
  fileName.textContent = path.split(/[/\\]/).pop();  // 仅显示文件名，不含路径
  filePlaceholder.classList.add("hidden");            // 隐藏虚线占位区
  fileSelected.classList.remove("hidden");            // 显示文件名条
  btnExtract.disabled = false;                        // 启用提取按钮
  hideStatus();
  btnOpenFolder.classList.add("hidden");

  await loadAudioInfo(path);  // 异步加载音频元数据
}

// 调用 Rust 后端的 get_audio_info → 解析 ffprobe 输出的 key=value 文本 → 渲染中文标签
async function loadAudioInfo(path) {
  try {
    const info = await invoke("get_audio_info", { inputPath: path });
    const parsed = {};
    info.split("\n").forEach(line => {
      const eq = line.indexOf("=");
      if (eq > 0) parsed[line.substring(0, eq).trim()] = line.substring(eq + 1).trim();
    });

    if (Object.keys(parsed).length > 0) {
      infoGrid.innerHTML = "";
      // ffprobe 原始字段 → 中文标签映射
      const labels = {
        codec_name: "编码格式",
        codec_long_name: "编码器",
        sample_rate: "采样率",
        channels: "声道数",
        bit_rate: "比特率"
      };

      Object.entries(labels).forEach(([key, label]) => {
        if (parsed[key]) {
          let value = parsed[key];
          if (key === "sample_rate") value = (parseInt(value) / 1000).toFixed(1) + " kHz";
          if (key === "bit_rate") value = (parseInt(value) / 1000).toFixed(0) + " kbps";
          infoGrid.innerHTML +=
            `<div class="info-item">
              <span class="info-label">${label}</span>
              <span class="info-value">${value}</span>
            </div>`;
        }
      });
      audioInfo.classList.remove("hidden");
    }
  } catch {
    audioInfo.classList.add("hidden");
  }
}

// 重置视频提取模块的 UI 状态
function resetFile() {
  selectedFilePath = null;
  fileName.textContent = "";
  filePlaceholder.classList.remove("hidden");
  fileSelected.classList.add("hidden");
  btnExtract.disabled = true;
  audioInfo.classList.add("hidden");
  hideStatus();
  btnOpenFolder.classList.add("hidden");
}

// 切换加载态：提取/转换过程中禁用所有交互元素，防止重复操作
function setLoading(loading) {
  btnExtract.disabled = loading;
  btnSelect.disabled = loading;
  btnClear.disabled = loading;

  if (loading) {
    btnExtractText.textContent = "提取中...";
    spinner.classList.remove("hidden");
  } else {
    btnExtractText.textContent = "提取音频";
    spinner.classList.add("hidden");
  }
}

// 显示状态提示（成功=绿色 ✅ / 错误=红色 ❌）
function showStatus(type, message) {
  statusArea.classList.remove("hidden", "success", "error");
  statusArea.classList.add(type);
  statusIcon.textContent = type === "success" ? "✅" : "❌";
  statusText.textContent = message;
}

function hideStatus() { statusArea.classList.add("hidden"); }

/* ===================================================================
   模块二：音频格式转换
   功能：选择 WAV/FLAC/MP3 → 三选一目标格式 → 一键转换
   设计思路与视频提取高度类似，但增加了格式选择器和同格式禁用逻辑
   =================================================================== */

// ---- DOM 元素 ----
const btnConvertSelect = document.getElementById("btnConvertSelect");       // 转换区"选择文件"
const btnConvertClear = document.getElementById("btnConvertClear");         // 转换区 ✕ 清除
const btnConvert = document.getElementById("btnConvert");                   // "转换格式" 主按钮
const btnConvertText = document.getElementById("btnConvertText");
const convertSpinner = document.getElementById("convertSpinner");
const convertFilePlaceholder = document.getElementById("convertFilePlaceholder");
const convertFileSelected = document.getElementById("convertFileSelected");
const convertFileName = document.getElementById("convertFileName");
const convertStatusArea = document.getElementById("convertStatusArea");
const convertStatusIcon = document.getElementById("convertStatusIcon");
const convertStatusText = document.getElementById("convertStatusText");
const btnConvertOpenFolder = document.getElementById("btnConvertOpenFolder");
const formatRadios = document.querySelectorAll("input[name='convertFormat']"); // 三个 radio 按钮

// ---- 事件绑定 ----

btnConvertSelect.addEventListener("click", async () => {
  try {
    const path = await invoke("pick_audio_file");  // 调用 Rust 弹出音频文件对话框
    if (path) setConvertFile(path);
  } catch (e) {
    showConvertStatus("error", "打开文件对话框失败: " + e);
  }
});

btnConvertClear.addEventListener("click", () => resetConvertFile());

// 每个 radio 切换时，检查是否需要禁用按钮（同格式则禁用）
formatRadios.forEach(radio => radio.addEventListener("change", updateConvertButtonState));

btnConvert.addEventListener("click", async () => {
  if (!selectedConvertPath) return;

  // 获取当前选中的目标格式
  const targetFormat = document.querySelector("input[name='convertFormat']:checked")?.value;
  if (!targetFormat) return;  // 防御：理论上 radio 总有一个被选中

  setConvertLoading(true);
  hideConvertStatus();

  try {
    const outputPath = await invoke("convert_audio", {
      inputPath: selectedConvertPath,
      targetFormat: targetFormat
    });
    showConvertStatus("success", "转换成功！已保存至: " + outputPath);
    btnConvertOpenFolder.classList.remove("hidden");
    btnConvertOpenFolder.dataset.outputPath = outputPath;
  } catch (e) {
    showConvertStatus("error", "转换失败: " + e);
  } finally {
    setConvertLoading(false);
  }
});

btnConvertOpenFolder.addEventListener("click", () => {
  const path = btnConvertOpenFolder.dataset.outputPath;
  if (path) invoke("open_folder", { path }).catch(() => {});
});

// ---- 内部函数 ----

function setConvertFile(path) {
  selectedConvertPath = path;
  convertFileName.textContent = path.split(/[/\\]/).pop();
  convertFilePlaceholder.classList.add("hidden");
  convertFileSelected.classList.remove("hidden");
  hideConvertStatus();
  btnConvertOpenFolder.classList.add("hidden");
  updateConvertButtonState();  // 更新按钮状态（检查是否同格式）
}

function resetConvertFile() {
  selectedConvertPath = null;
  convertFileName.textContent = "";
  convertFilePlaceholder.classList.remove("hidden");
  convertFileSelected.classList.add("hidden");
  btnConvert.disabled = true;
  hideConvertStatus();
  btnConvertOpenFolder.classList.add("hidden");
}

// 同格式禁用检测：若源格式 == 目标格式，禁用转换按钮
// 触发时机：选择文件后、切换 radio 时
function updateConvertButtonState() {
  const targetFormat = document.querySelector("input[name='convertFormat']:checked")?.value;
  const srcExt = selectedConvertPath
    ? selectedConvertPath.split(".").pop()?.toLowerCase()
    : null;

  btnConvert.disabled = !(
    selectedConvertPath          // 已选择文件
    && targetFormat              // 已选择目标格式（radio 默认选中 wav）
    && srcExt !== targetFormat   // 源 ≠ 目标（避免无意义转换）
  );
}

function setConvertLoading(loading) {
  btnConvert.disabled = loading;
  btnConvertSelect.disabled = loading;
  btnConvertClear.disabled = loading;

  if (loading) {
    btnConvertText.textContent = "转换中...";
    convertSpinner.classList.remove("hidden");
  } else {
    btnConvertText.textContent = "转换格式";
    convertSpinner.classList.add("hidden");
    updateConvertButtonState();  // 恢复时重新检查按钮状态
  }
}

function showConvertStatus(type, message) {
  convertStatusArea.classList.remove("hidden", "success", "error");
  convertStatusArea.classList.add(type);
  convertStatusIcon.textContent = type === "success" ? "✅" : "❌";
  convertStatusText.textContent = message;
}

function hideConvertStatus() { convertStatusArea.classList.add("hidden"); }

/* ===================================================================
   模块三：音频分析
   功能：拖拽/选择 WAV/FLAC/MP3 → 可选填文字 → 信号分析 → Canvas 图表渲染
   架构：独立子视图（analysisView），通过主页导航卡片进入
   =================================================================== */

// ---- DOM 元素 ----
const dropzonePlaceholder = document.getElementById("dropzonePlaceholder");     // 拖拽区（未选择时）
const analysisFileSelected = document.getElementById("analysisFileSelected");   // 已选择文件条
const analysisFileName = document.getElementById("analysisFileName");
const btnAnalysisSelect = document.getElementById("btnAnalysisSelect");          // "选择文件" 按钮
const btnAnalysisClear = document.getElementById("btnAnalysisClear");            // ✕ 清除
const btnAnalyze = document.getElementById("btnAnalyze");                        // "开始分析" 主按钮
const btnAnalyzeText = document.getElementById("btnAnalyzeText");
const analysisSpinner = document.getElementById("analysisSpinner");
const analysisStatusArea = document.getElementById("analysisStatusArea");
const analysisStatusIcon = document.getElementById("analysisStatusIcon");
const analysisStatusText = document.getElementById("analysisStatusText");
const transcriptText = document.getElementById("transcriptText");                // 文字输入框
const chartsArea = document.getElementById("chartsArea");                        // 图表总容器

// ---- 事件绑定 ----

// 方式一：点击"选择文件" → 原生对话框
btnAnalysisSelect.addEventListener("click", async () => {
  try {
    const path = await invoke("pick_analysis_file");
    if (path) setAnalysisFile(path);
  } catch (e) {
    showAnalysisStatus("error", "打开文件对话框失败: " + e);
  }
});

btnAnalysisClear.addEventListener("click", resetAnalysis);

// 点击"开始分析" → 调用 Rust 分析引擎 → 获取 JSON → 渲染 Canvas 图表
btnAnalyze.addEventListener("click", async () => {
  if (!selectedAnalysisPath) return;
  setAnalysisLoading(true);
  hideAnalysisStatus();

  const text = transcriptText.value.trim() || null;  // 空字符串视为 null

  try {
    // 调用 Rust 后端分析引擎，返回 JSON 字符串
    const resultJson = await invoke("analyze_audio", {
      inputPath: selectedAnalysisPath,
      transcript: text
    });
    const data = JSON.parse(resultJson);  // 解析 JSON
    chartsArea.classList.remove("hidden"); // 先显示容器，让 Canvas 获得正确的布局尺寸
    renderAllCharts(data);                 // 触发所有 Canvas 渲染
    chartsArea.scrollIntoView({ behavior: "smooth" });  // 平滑滚动到图表区
  } catch (e) {
    showAnalysisStatus("error", "分析失败: " + e);
  } finally {
    setAnalysisLoading(false);
  }
});

/* ------ 拖拽文件处理 ------ */

const analysisDropzone = document.getElementById("analysisDropzone");

// dragover: 必须 preventDefault() 才能允许 drop 事件触发
analysisDropzone.addEventListener("dragover", (e) => {
  e.preventDefault();
  e.stopPropagation();
  analysisDropzone.classList.add("dragover");        // 高亮拖拽区域
  dropzonePlaceholder.classList.add("dragover");
});

// dragleave: 文件拖出区域 → 取消高亮
analysisDropzone.addEventListener("dragleave", (e) => {
  e.preventDefault();
  e.stopPropagation();
  analysisDropzone.classList.remove("dragover");
  dropzonePlaceholder.classList.remove("dragover");
});

// drop: 文件释放 → 提取路径并处理
analysisDropzone.addEventListener("drop", (e) => {
  e.preventDefault();
  e.stopPropagation();
  analysisDropzone.classList.remove("dragover");
  dropzonePlaceholder.classList.remove("dragover");

  const files = e.dataTransfer?.files;  // 拖入的文件列表
  if (files && files.length > 0) {
    const file = files[0];
    if (file.name) {
      const ext = file.name.split(".").pop()?.toLowerCase();
      // 格式校验：只接受 wav/flac/mp3
      if (["wav", "flac", "mp3"].includes(ext)) {
        setAnalysisFile(file.path || file.name);
      } else {
        showAnalysisStatus("error", "不支持的文件格式，请拖入 WAV / FLAC / MP3 文件");
      }
    }
  }
});

// ---- 内部函数 ----

function setAnalysisFile(path) {
  selectedAnalysisPath = path;
  analysisFileName.textContent = path.split(/[/\\]/).pop();
  dropzonePlaceholder.classList.add("hidden");       // 隐藏拖拽区
  analysisFileSelected.classList.remove("hidden");   // 显示文件名条
  btnAnalyze.disabled = false;                       // 启用分析按钮
  hideAnalysisStatus();
  chartsArea.classList.add("hidden");                // 隐藏旧图表
}

// 重置分析页：清空文件、文字、图表、状态
function resetAnalysis() {
  selectedAnalysisPath = null;
  analysisFileName.textContent = "";
  dropzonePlaceholder.classList.remove("hidden");
  analysisFileSelected.classList.add("hidden");
  btnAnalyze.disabled = true;
  hideAnalysisStatus();
  chartsArea.classList.add("hidden");
  transcriptText.value = "";
}

function setAnalysisLoading(loading) {
  btnAnalyze.disabled = loading;
  btnAnalysisSelect.disabled = loading;
  btnAnalysisClear.disabled = loading;

  if (loading) {
    btnAnalyzeText.textContent = "分析中...";
    analysisSpinner.classList.remove("hidden");
  } else {
    btnAnalyzeText.textContent = "开始分析";
    analysisSpinner.classList.add("hidden");
  }
}

function showAnalysisStatus(type, message) {
  analysisStatusArea.classList.remove("hidden", "success", "error");
  analysisStatusArea.classList.add(type);
  analysisStatusIcon.textContent = type === "success" ? "✅" : "❌";
  analysisStatusText.textContent = message;
}

function hideAnalysisStatus() { analysisStatusArea.classList.add("hidden"); }

/* ===================================================================
   Canvas 图表渲染引擎
   所有图表手写 Canvas API，零外部依赖。
   每个 render* 函数负责一种图表类型的渲染。
   所有图表的 X 轴统一映射为时间轴（0 → duration 秒）。
   =================================================================== */

/**
 * Canvas 初始化工具函数
 *
 * @param {string} id - canvas 元素的 id
 * @returns {{ ctx: CanvasRenderingContext2D, w: number, h: number }}
 *
 * 关键：使用 devicePixelRatio 缩放以确保在 HiDPI 屏幕（Retina/2K/4K）上清晰渲染。
 * 物理像素 = CSS 像素 × devicePixelRatio
 */
function initCanvas(id) {
  const canvas = document.getElementById(id);
  const rect = canvas.getBoundingClientRect();           // CSS 像素宽高
  const dpr = window.devicePixelRatio || 1;              // 设备像素比（普通屏=1, Retina=2）
  canvas.width = rect.width * dpr;                       // 设置物理像素宽度
  canvas.height = rect.height * dpr;
  const ctx = canvas.getContext("2d");
  ctx.scale(dpr, dpr);                                   // 缩放坐标系，后续所有操作使用 CSS 像素
  ctx.clearRect(0, 0, rect.width, rect.height);          // 清空画布
  return { ctx, w: rect.width, h: rect.height };
}

// 编排所有图表的渲染，根据数据内容决定显示哪些图表
function renderAllCharts(data) {
  renderWaveform(data);
  renderSpectrogram(data);
  renderPitch(data);
  renderLoudness(data);

  // 文字时序图表：whisper ASR 结果（数组）或错误对象时都显示
  const ta = data.text_alignment;
  if (ta) {
    if (Array.isArray(ta) && ta.length > 0) {
      document.getElementById("chartTextSection").classList.remove("hidden");
      renderTextAlignment(data);
    } else if (ta.error) {
      document.getElementById("chartTextSection").classList.remove("hidden");
      const canvas = document.getElementById("canvasTextAlignment");
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      const ctx = canvas.getContext("2d");
      ctx.scale(dpr, dpr);
      ctx.clearRect(0, 0, rect.width, rect.height);
      ctx.fillStyle = "#8888aa";
      ctx.font = "14px sans-serif";
      ctx.textAlign = "center";
      ctx.fillText("⚠ whisper 未安装，无法进行语音识别", rect.width / 2, rect.height / 2 - 10);
      ctx.font = "11px sans-serif";
      ctx.fillText("请运行 tools\\setup-whisper.ps1 下载安装", rect.width / 2, rect.height / 2 + 14);
    } else {
      document.getElementById("chartTextSection").classList.add("hidden");
    }
  } else {
    document.getElementById("chartTextSection").classList.add("hidden");
  }
}

/* ------ 波形图 (Waveform) ------ */

/**
 * 绘制波形图
 *
 * 数据：data.waveform — 1200 个降采样点，归一化幅度值
 * 视觉：紫色折线 + 浅紫色水平中线，直观展示声音振幅随时间的变化
 * 底部：5 等分时间轴刻度
 */
function renderWaveform(data) {
  const { ctx, w, h } = initCanvas("canvasWaveform");
  const points = data.waveform || [];
  if (points.length === 0) return;

  const mid = h / 2;  // 水平中线位置（波形的对称轴）
  const maxVal = points.reduce((a, b) => Math.max(a, Math.abs(b)), 0.001); // 全局最大幅度

  // 绘制波形折线
  ctx.strokeStyle = "#6c63ff";   // 主题紫色
  ctx.lineWidth = 1.2;
  ctx.beginPath();
  points.forEach((v, i) => {
    const x = (i / (points.length - 1)) * w;       // X 轴等距映射
    const y = mid - (v / maxVal) * mid * 0.9;       // Y 轴：正值向上，负值向下
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.stroke();

  // 绘制水平中线（零线参考）
  ctx.strokeStyle = "rgba(108,99,255,0.3)";
  ctx.lineWidth = 0.5;
  ctx.beginPath();
  ctx.moveTo(0, mid);
  ctx.lineTo(w, mid);
  ctx.stroke();

  // 底部时间轴刻度
  const dur = data.duration || 0;
  ctx.fillStyle = "#666688";
  ctx.font = "10px sans-serif";
  const ticks = 5;
  for (let i = 0; i <= ticks; i++) {
    const x = (i / ticks) * w;
    const t = (i / ticks) * dur;
    ctx.fillText(t.toFixed(1) + "s", x - 12, h - 4);
  }
}

/* ------ 频谱图 (Spectrogram) ------ */

/**
 * 绘制频谱图（热力图）
 *
 * 数据：data.spectrogram — { times, frequencies, magnitudes }
 *       magnitudes 是 times.length × 128 的二维矩阵
 *
 * 绘制方式：逐像素计算颜色 → createImageData → putImageData
 *           比 fillRect 逐个绘制快得多
 *
 * 视觉：深蓝（安静）→ 紫 → 明红（高能量）
 * Y 轴：底部 = 低频，顶部 = 高频
 */
function renderSpectrogram(data) {
  const { ctx, w, h } = initCanvas("canvasSpectrogram");
  const spec = data.spectrogram;
  if (!spec || !spec.magnitudes || spec.magnitudes.length === 0) return;

  const times = spec.times;
  const freqList = spec.frequencies;
  const mags = spec.magnitudes;

  // 全局最大值（用于归一化）
  const maxMag = mags.flat().reduce((a, b) => Math.max(a, b), 0.001);

  // 创建像素缓冲区
  const imageData = ctx.createImageData(w, h);

  // 逐像素渲染（px = 列 = 时间, py = 行 = 频率）
  for (let px = 0; px < w; px++) {
    const ti = Math.floor((px / w) * times.length);
    const tIdx = Math.min(ti, times.length - 1);

    for (let py = 0; py < h; py++) {
      // Y 轴翻转：py=0 是顶部（高频），py=h-1 是底部（低频）
      const fi = Math.floor(((h - 1 - py) / (h - 1)) * freqList.length);
      const fIdx = Math.min(fi, freqList.length - 1);
      const val = mags[tIdx] ? (mags[tIdx][fIdx] || 0) / maxMag : 0;

      const idx = (py * w + px) * 4;  // RGBA 四通道起始索引

      // 仿 magma 热力颜色映射：黑（无能量）→ 深紫 → 橙红 → 亮黄（峰值）
      // 使用非线性映射 pow(val, 0.6) 提升低能量区域的可见性
      const v = Math.pow(val, 0.6);
      let rr, gg, bb;

      if (v < 0.25) {
        rr = v * 4 * 80;
        gg = 0;
        bb = v * 4 * 180;
      } else if (v < 0.5) {
        const t = (v - 0.25) * 4;
        rr = 80 + t * 120;
        gg = t * 80;
        bb = 180 - t * 170;
      } else if (v < 0.75) {
        const t = (v - 0.5) * 4;
        rr = 200 + t * 55;
        gg = 80 + t * 175;
        bb = 10 - t * 10;
      } else {
        const t = (v - 0.75) * 4;
        rr = 255;
        gg = 255 * t + 200 * (1 - t);
        bb = 255 * t * 0.8;
      }

      imageData.data[idx]     = Math.floor(rr);       // R
      imageData.data[idx + 1] = Math.floor(gg);        // G
      imageData.data[idx + 2] = Math.floor(bb);        // B
      imageData.data[idx + 3] = 255;                   // A（不透明）
    }
  }
  ctx.putImageData(imageData, 0, 0);

  // 底部时间轴刻度
  const dur = data.duration || 0;
  ctx.fillStyle = "#8888aa";
  ctx.font = "10px sans-serif";
  const ticks = 5;
  for (let i = 0; i <= ticks; i++) {
    const x = (i / ticks) * w;
    ctx.fillText(((i / ticks) * dur).toFixed(1) + "s", x - 12, h - 4);
  }
}

/* ------ 音高曲线 (Pitch) ------ */

/**
 * 绘制音高曲线
 *
 * 数据：data.pitch — { times, values }
 *       values 单位 Hz，0 表示无声段
 *
 * Y 轴范围：0-500Hz，每 100Hz 一个刻度
 * 颜色：青绿色 (#22d3bb)
 */
function renderPitch(data) {
  const { ctx, w, h } = initCanvas("canvasPitch");
  const pitch = data.pitch;
  if (!pitch || !pitch.times || pitch.times.length === 0) return;

  const times = pitch.times;
  const values = pitch.values;
  const dur = data.duration || 0;

  // 背景底色
  ctx.fillStyle = "rgba(0,0,0,0.15)";
  ctx.fillRect(0, 0, w, h);

  // 绘制音高折线
  ctx.strokeStyle = "#22d3bb";  // 青绿色
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  for (let i = 0; i < values.length; i++) {
    const x = (times[i] / dur) * w;
    const clamped = Math.min(Math.max(values[i], 0), 500);  // 限制在 0-500Hz 范围内
    const y = h - (clamped / 500) * h * 0.9 - h * 0.05;
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }
  ctx.stroke();

  // Y 轴刻度（Hz）
  ctx.fillStyle = "#8888aa";
  ctx.font = "10px sans-serif";
  for (let i = 0; i <= 5; i++) {
    const y = h - (i / 5) * h * 0.9 - h * 0.05;
    ctx.fillText(Math.round(i * 100) + "Hz", 2, y - 2);
  }
  // X 轴（秒）
  const ticks = 5;
  for (let i = 0; i <= ticks; i++) {
    const x = (i / ticks) * w;
    ctx.fillText(((i / ticks) * dur).toFixed(1) + "s", x - 12, h - 4);
  }
}

/* ------ 响度曲线 (Loudness) ------ */

/**
 * 绘制响度曲线
 *
 * 数据：data.loudness — { times, values }
 *       values 单位 dB
 *
 * 绘制方式：金色半透明面积填充 + 金色实线轮廓
 * Y 轴：dB（动态范围，根据实际最小值确定下限）
 */
function renderLoudness(data) {
  const { ctx, w, h } = initCanvas("canvasLoudness");
  const loudness = data.loudness;
  if (!loudness || !loudness.times || loudness.times.length === 0) return;

  const times = loudness.times;
  const values = loudness.values;
  const dur = data.duration || 0;

  // 动态 Y 轴范围
  const minDb = Math.min(...values.filter(v => v > -90), -30);
  const range = -minDb;

  // 先绘制半透明面积填充
  ctx.fillStyle = "rgba(234, 179, 8, 0.25)";  // 金色半透明
  ctx.beginPath();
  ctx.moveTo(0, h);  // 从左下角开始
  for (let i = 0; i < values.length; i++) {
    const x = (times[i] / dur) * w;
    const clamped = Math.max(values[i], minDb);
    const y = h - ((clamped - minDb) / range) * h * 0.85 - h * 0.08;
    ctx.lineTo(x, y);
  }
  ctx.lineTo(w, h);  // 到右下角
  ctx.closePath();   // 回到左下角形成闭合区域
  ctx.fill();

  // 再绘制轮廓线
  ctx.strokeStyle = "#eab308";  // 金色
  ctx.lineWidth = 1.2;
  ctx.beginPath();
  for (let i = 0; i < values.length; i++) {
    const x = (times[i] / dur) * w;
    const clamped = Math.max(values[i], minDb);
    const y = h - ((clamped - minDb) / range) * h * 0.85 - h * 0.08;
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }
  ctx.stroke();

  // Y 轴（dB）
  ctx.fillStyle = "#8888aa";
  ctx.font = "10px sans-serif";
  for (let i = 0; i <= 3; i++) {
    const db = minDb + (i / 3) * range;
    const y = h - (i / 3) * h * 0.85 - h * 0.08;
    ctx.fillText(Math.round(db) + "dB", 2, y - 2);
  }
  // X 轴（秒）
  const ticks = 5;
  for (let i = 0; i <= ticks; i++) {
    const x = (i / ticks) * w;
    ctx.fillText(((i / ticks) * dur).toFixed(1) + "s", x - 12, h - 4);
  }
}

/* ------ 文字时序对齐 (Text Alignment) ------ */

/**
 * 绘制文字时序图
 *
 * 数据：data.text_alignment — [{ text, start, end }, ...]
 *
 * 渲染方式：每个文字/音素段绘制为彩色矩形，标注文字内容
 * 配色：6 色循环（紫/青/金/红/绿/蓝）区分相邻字
 * 合并规则：相邻段若文字相同且时间连续则合并为一个显示块
 */
function renderTextAlignment(data) {
  const { ctx, w, h } = initCanvas("canvasTextAlignment");
  const items = data.text_alignment || [];
  if (items.length === 0) return;

  const dur = data.duration || 0;

  // 合并相邻同文字段
  const labels = [];
  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (labels.length === 0
      || labels[labels.length - 1].text !== item.text
      || item.end !== labels[labels.length - 1].start) {
      labels.push({ text: item.text, start: item.start, end: item.end });
    } else {
      labels[labels.length - 1].end = item.end;  // 扩展前一段
    }
  }

  // 6 色循环配色
  const colors = [
    "rgba(108,99,255,0.5)",   // 紫
    "rgba(34,211,187,0.5)",   // 青
    "rgba(234,179,8,0.5)",    // 金
    "rgba(239,68,68,0.5)",    // 红
    "rgba(16,185,129,0.5)",   // 绿
    "rgba(59,130,246,0.5)"    // 蓝
  ];

  ctx.fillStyle = "#555577";
  ctx.font = "11px sans-serif";

  labels.forEach((item, i) => {
    const x1 = (item.start / dur) * w;
    const x2 = (item.end / dur) * w;
    const color = colors[i % colors.length];

    // 绘制彩色矩形块
    ctx.fillStyle = color;
    ctx.fillRect(x1, 8, x2 - x1, h - 24);

    // 仅在宽度足够时标注文字（太窄显示不下）
    ctx.fillStyle = "#ffffff";
    if (x2 - x1 > 20) {
      ctx.fillText(item.text, x1 + 3, h / 2 + 4);
    }
  });

  // 底部时间轴刻度
  ctx.strokeStyle = "rgba(255,255,255,0.15)";
  ctx.lineWidth = 0.5;
  ctx.beginPath();
  ctx.moveTo(0, h - 12);
  ctx.lineTo(w, h - 12);
  ctx.stroke();

  ctx.fillStyle = "#8888aa";
  ctx.font = "10px sans-serif";
  const ticks = 6;
  for (let i = 0; i <= ticks; i++) {
    const x = (i / ticks) * w;
    ctx.fillText(((i / ticks) * dur).toFixed(1) + "s", x - 12, h - 1);
  }
}
