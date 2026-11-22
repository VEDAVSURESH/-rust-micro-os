@echo off
setlocal enabledelayedexpansion

:: ==============================================================================
:: JainOS 1-Click VirtualBox Automated Launcher
:: Author: Vedav Suresh (USN: 25BTRCL280, Section: CSE AIML-E)
:: Project: JainOS — Rust Micro Operating System
:: Target: x86_64 Bare-Metal no_std Microkernel
:: ==============================================================================

title JainOS Micro Operating System - VirtualBox Launcher
color 0B

echo.
echo  =============================================================================
echo                 JAINOS — RUST MICRO OPERATING SYSTEM                          
echo     Author: Vedav Suresh  ^|  USN: 25BTRCL280  ^|  Section: CSE AIML-E        
echo  =============================================================================
echo.
echo   [*] Features:
echo       - Pure #![no_std] x86_64 Rust Microkernel (Single OS)
echo       - GDT, TSS, IDT, PIC, PIT Timer (100 Hz), CMOS RTC
echo       - Safe Physical Memory & 100 KiB Heap Allocator
echo       - Preemptive / Cooperative Task Scheduler
echo       - PCI Bus Enumerator & Intel PRO/1000 MT (e1000) Gigabit NIC
echo       - Native TCP/IP Network Stack (DHCP, ARP, IPv4, ICMP ping, TCP HTTP)
echo       - Standalone Bootable ISO Image ^< 100 MB (~1.4 MB)
echo.

:: 1. Locate VBoxManage
set "VBOX_BIN="
if exist "%ProgramFiles%\Oracle\VirtualBox\VBoxManage.exe" (
    set "VBOX_BIN=%ProgramFiles%\Oracle\VirtualBox\VBoxManage.exe"
) else if exist "%ProgramFiles(x86)%\Oracle\VirtualBox\VBoxManage.exe" (
    set "VBOX_BIN=%ProgramFiles(x86)%\Oracle\VirtualBox\VBoxManage.exe"
) else if defined VBOX_MSI_INSTALL_PATH (
    if exist "!VBOX_MSI_INSTALL_PATH!VBoxManage.exe" set "VBOX_BIN=!VBOX_MSI_INSTALL_PATH!VBoxManage.exe"
) else if defined VBOX_INSTALL_PATH (
    if exist "!VBOX_INSTALL_PATH!VBoxManage.exe" set "VBOX_BIN=!VBOX_INSTALL_PATH!VBoxManage.exe"
) else (
    where VBoxManage.exe >nul 2>&1
    if !errorlevel! equ 0 set "VBOX_BIN=VBoxManage.exe"
)

if "%VBOX_BIN%"=="" (
    color 0C
    echo [ERROR] Oracle VM VirtualBox is not detected on your system.
    echo Please install VirtualBox from https://www.virtualbox.org/ or use QEMU.
    echo.
    pause
    exit /b 1
)

echo [OK] VirtualBox Engine: "%VBOX_BIN%"

:: 2. Locate JainOS Bootable ISO
set "ISO_PATH="
if exist "%~dp0build\JainOS.iso" (
    set "ISO_PATH=%~dp0build\JainOS.iso"
) else if exist "%~dp0JainOS.iso" (
    set "ISO_PATH=%~dp0JainOS.iso"
) else if exist "\\wsl.localhost\Ubuntu-22.04\home\vedav\rust-micro-os\build\JainOS.iso" (
    set "ISO_PATH=\\wsl.localhost\Ubuntu-22.04\home\vedav\rust-micro-os\build\JainOS.iso"
)

if "%ISO_PATH%"=="" (
    color 0E
    echo [WARNING] JainOS.iso was not found in .\build\.
    echo Building the ISO automatically via WSL...
    wsl -d Ubuntu-22.04 -- bash -l -c "/home/vedav/rust-micro-os/build/build_jainos_iso.sh"
    if exist "%~dp0build\JainOS.iso" set "ISO_PATH=%~dp0build\JainOS.iso"
)

if "%ISO_PATH%"=="" (
    echo [ERROR] Unable to locate or build JainOS.iso.
    pause
    exit /b 1
)

echo [OK] Bootable JainOS ISO: "%ISO_PATH%"

:: 3. VM Provisioning Configuration
set "VM_NAME=JainOS"

echo.
echo [*] Checking VirtualBox VM status for '%VM_NAME%'...
"%VBOX_BIN%" showvminfo "%VM_NAME%" >nul 2>&1
if %errorlevel% equ 0 (
    echo [OK] Existing VM detected. Updating media attachment...
    "%VBOX_BIN%" storageattach "%VM_NAME%" --storagectl "IDE Controller" --port 0 --device 0 --type dvddrive --medium "%ISO_PATH%" >nul 2>&1
) else (
    echo [*] Provisioning customized VM '%VM_NAME%'...
    "%VBOX_BIN%" createvm --name "%VM_NAME%" --ostype "Other_64" --register
    "%VBOX_BIN%" modifyvm "%VM_NAME%" --memory 1024 --cpus 1 --vram 16 --graphicscontroller vmsvga
    "%VBOX_BIN%" modifyvm "%VM_NAME%" --boot1 dvd --boot2 none --boot3 none --boot4 none
    "%VBOX_BIN%" modifyvm "%VM_NAME%" --nic1 nat --nictype1 82540EM --macaddress1 525400123456
    "%VBOX_BIN%" modifyvm "%VM_NAME%" --audio-enabled off
    "%VBOX_BIN%" modifyvm "%VM_NAME%" --mouse ps2

    "%VBOX_BIN%" storagectl "%VM_NAME%" --name "IDE Controller" --add ide
    "%VBOX_BIN%" storageattach "%VM_NAME%" --storagectl "IDE Controller" --port 0 --device 0 --type dvddrive --medium "%ISO_PATH%"
    echo [OK] VM '%VM_NAME%' provisioned successfully.
)

:: 4. Launch Virtual Machine
echo.
echo ===============================================================================
echo  STARTING JAINOS IN VIRTUALBOX...
echo ===============================================================================
"%VBOX_BIN%" startvm "%VM_NAME%"

echo.
echo [✓] VM launched successfully.
echo     - Active User: vedav
echo     - Networking: Intel PRO/1000 MT (e1000) over VirtualBox NAT
echo.
timeout /t 5 >nul
exit /b 0
