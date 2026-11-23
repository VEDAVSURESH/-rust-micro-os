@echo off
title JainOS Wi-Fi Bridge Service (Port 19820)
color 0b
echo ================================================================
echo           JainOS Host Wi-Fi Bridge Service (v0.3.4)
echo     Bridges Windows Host Wi-Fi to JainOS VirtualBox / QEMU
echo ================================================================
echo.
echo [+] Starting Host Wi-Fi Telemetry Server on 0.0.0.0:19820...
echo [+] Keep this window open while using JainOS in VirtualBox!
echo.

python tools\windows_host_wifi_helper.py
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [-] Python not found in PATH or failed to start.
    echo [+] Attempting PowerShell native helper fallback...
    powershell -ExecutionPolicy Bypass -File tools\windows_host_wifi_helper.ps1
)

echo.
echo Server stopped.
pause
