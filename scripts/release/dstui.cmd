@echo off
REM Fork 启动 wrapper：设置 DSTUI_HOME 和 DEEPSEEK_CONFIG_PATH
REM 安装方式：将此文件与 dstui.exe 放在同一目录，或放到 PATH 中

if not defined DSTUI_HOME (
    set "DSTUI_HOME=%USERPROFILE%\.dstui"
)

if not exist "%DSTUI_HOME%" (
    mkdir "%DSTUI_HOME%"
)

set "DEEPSEEK_CONFIG_PATH=%DSTUI_HOME%\config.toml"

REM 查找 dstui.exe：优先同目录，否则 PATH
set "DSTUI_BIN=%~dp0dstui.exe"
if not exist "%DSTUI_BIN%" (
    where dstui.exe >nul 2>&1
    if errorlevel 1 (
        echo 错误: 找不到 dstui.exe
        exit /b 1
    )
    set "DSTUI_BIN=dstui.exe"
)

"%DSTUI_BIN%" %*
