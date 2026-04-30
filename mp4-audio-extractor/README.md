# MP4 Audio Extractor

一个基于 Tauri 的 MP4 视频音频提取工具，支持无损提取音频流并转换为 WAV 格式。

## Features

- 🎵 **MP4 音频提取** - 无损提取 MP4 视频中的音频流
- 🔄 **音频格式转换** - 支持 WAV/FLAC/MP3 格式互转
- 📊 **音频分析** - 波形、频谱、音高、响度可视化
- 🎙️ **语音识别** - 集成 whisper.cpp 进行语音转文字（未完）

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

***

# Git 学习与实践记录

## 一、Git 学习资料来源

### 官方文档

- [Git 官网](https://git-scm.com/)
- [Git 官方文档（英文）](https://git-scm.com/doc)

***

## 二、Git 实践流程

### 1. 安装与配置

#### Windows 安装

```bash
# 下载安装包
https://git-scm.com/download/win

# 安装后打开 Git Bash 或 PowerShell
```

#### 初始配置

```bash
# 设置用户名和邮箱
git config --global user.name "Your Name"
git config --global user.email "your.email@example.com"

# 查看配置
git config --list
```

### 2. 仓库初始化

```bash
# 进入项目目录
cd project-path

# 初始化仓库
git init

# 查看状态
git status
```

### 3. 日常开发流程

```bash
# 1. 查看当前状态
git status

# 2. 添加文件到暂存区
git add .              # 添加所有文件
git add 文件路径        # 添加指定文件

# 3. 提交更改
git commit -m "提交信息"

# 4. 查看提交历史
git log --oneline

# 5. 推送到远程仓库
git push
```

### 4. GitHub 远程仓库操作

```bash
# 添加远程仓库
git remote add origin https://github.com/username/repo.git

# 首次推送并设置上游分支
git push -u origin main

# 后续推送
git push

# 从远程拉取更新
git pull
```

### 5. 实用查看命令

```bash
# 查看暂存区文件（确认将要提交的内容）
git ls-files --stage

# 查看远程仓库地址
git remote -v

# 取消暂存文件（不删除本地文件）
git rm --cached 文件路径
```

### 6. Windows PowerShell 注意事项

```bash
# ❌ 不支持 && 用作命令分隔符
cd "path" && git status  # 错误

# ✅ 使用 ; 分隔多个命令
cd "path"; git status   # 正确

# ✅ 或使用 & 连接符
cd "path"& git status   # 正确
```

***

## 三、提交记录

### 提交历史

| 提交      | 内容                      | 说明                                                |
| ------- | ----------------------- | ------------------------------------------------- |
| f048cfb | Initial commit          | 初始提交：mp4-audio-extractor 项目，包含 Tauri GUI 和 CLI 版本 |
| 94a2fc8 | Remove development docs | 移除开发文档（功能更新、开发文档等），避免暴露隐私                         |
| a6efe79 | Add README.md           | 添加项目说明文档                                          |

### 提交信息规范

```
feat:     新功能
fix:      修复 bug
docs:     文档更新
style:    代码格式调整
refactor: 重构
test:     测试相关
chore:    构建/工具相关
```

***

## 四、遇到的问题及解决方法

### 问题 1：.gitignore 不生效

**现象**：
修改 `.gitignore` 文件后，文件仍然被 git 追踪和提交。

**原因**：

- `.gitignore` 只会对尚未被 git 追踪的文件生效
- 已追踪的文件修改 `.gitignore` 后不会自动停止追踪

**解决方法**：

```bash
# 1. 从暂存区移除文件（不删除本地文件）
git rm --cached 文件路径

# 2. 确认文件已从暂存区移除
git status

# 3. 提交更改
git commit -m "Remove file from tracking"
```

### 问题 2：强制推送覆盖了远程内容

**现象**：
使用 `git push -f` 后，GitHub 上新建的 README.md 和其他提交被覆盖消失。

**原因**：

- 强制推送会用本地版本完全覆盖远程仓库
- 如果远程有本地没有的提交，这些提交会丢失

**解决方法**：

```bash
# 避免使用强制推送，除非必须
# 如果需要恢复，可以从 GitHub 找到之前的提交记录

# 正确流程：
# 1. 先拉取远程最新代码
git pull

# 2. 合并或变基后推送
git push

# 3. 如果必须强制推送，先确认远程内容已同步
git push -f origin main
```

### 问题 3：嵌套 Git 仓库导致无法提交

**现象**：
执行 `git add .` 时报错：`'01/' does not have a commit checked out`

**原因**：
某个子目录（如 `01/`）本身是一个独立的 git 仓库，git 无法将它作为普通文件提交。

**解决方法**：

```bash
# 1. 将子仓库添加到 .gitignore
echo "01/" >> .gitignore

# 2. 提交 .gitignore 更新
git add .gitignore
git commit -m "Ignore nested repository"

# 3. 确认状态
git status
```

### 问题 4：IDE 提交一直卡住

**现象**：
在 IDE（如 VS Code、Trae）中点击提交，进度一直不动。

**原因**：
IDE 的 Git 插件可能存在兼容性问题或性能问题。

**解决方法**：

```bash
# 使用命令行提交，更可靠
git add .
git commit -m "你的提交信息"
git push
```

***

## 五、Git 学习心得

### 1. 理解工作区、暂存区、版本库

Git 有三个关键区域：

- **工作区（Working Directory）**：实际操作的文件夹
- **暂存区（Stage/Index）**：准备提交的文件快照
- **版本库（Repository）**：提交的版本历史

```
工作区 → git add → 暂存区 → git commit → 版本库
```

### 2. .gitignore 关键原则

- **位置**：只有仓库根目录的 `.gitignore` 生效，子目录的不会生效
- **时机**：只对未追踪文件有效，已追踪文件需先移除
- **生效**：修改 `.gitignore` 后，已追踪文件不会自动消失

### 3. 提交前先检查

养成习惯：`git status` 查看当前状态，确认要提交的文件。

```bash
# 查看将要提交的内容
git status
git ls-files --stage  # 更详细查看暂存区
```

### 4. 提交信息要清晰

使用有意义的提交信息，方便回顾历史和协作。

```
feat:     新功能
fix:      修复 bug
docs:     文档更新
refactor: 重构
```

### 5. 远程操作要谨慎

- 推送前确认本地版本是最新的
- 避免使用 `git push -f`
- 多人协作时使用 Pull Request 而非直接推送 main 分支
- **敏感文件（密钥、token）绝不提交**

### 6. 分支策略

```bash
# 创建新分支
git checkout -b feature-name

# 切换分支
git checkout main

# 合并分支
git merge feature-name

# 删除分支
git branch -d feature-name
```

### 7. 安全操作流程

```bash
# 1. 查看当前状态
git status

# 2. 添加需要的文件
git add 需要的文件

# 3. 确认暂存区内容
git ls-files --stage

# 4. 提交
git commit -m "描述你的更改"

# 5. 推送
git push
```

***

## License

MIT
