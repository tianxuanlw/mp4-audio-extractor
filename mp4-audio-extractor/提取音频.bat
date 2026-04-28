@echo off
chcp 65001 >nul
title MP4 音频提取器

if "%~1"=="" (
    echo MP4 音频无损提取工具
    echo.
    echo 用法：
    echo   直接将 .mp4 文件拖拽到此脚本上即可
    echo.
    echo   或在命令行中运行：
    echo   mp4-audio-extractor-cli.exe video.mp4
    echo.
    echo   支持一次拖拽多个文件
    echo.
    pause
    exit /b
)

set EXE_PATH=%~dp0mp4-audio-extractor-cli.exe

if not exist "%EXE_PATH%" (
    echo [错误] 找不到 mp4-audio-extractor-cli.exe
    echo 请确保 exe 与此脚本在同一目录
    pause
    exit /b 1
)

set FILE_LIST=
:loop
if "%~1"=="" goto run
set FILE_LIST=%FILE_LIST% "%~1"
shift
goto loop

:run
"%EXE_PATH%" %FILE_LIST%
echo.
echo 按任意键关闭...
pause >nul
