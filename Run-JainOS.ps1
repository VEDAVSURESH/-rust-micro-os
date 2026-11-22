<#
==============================================================================
 JainOS 1-Click PowerShell VirtualBox Launcher
 Author: Vedav Suresh (USN: 25BTRCL280, Section: CSE AIML-E)
 Project: JainOS — Rust Micro Operating System
==============================================================================
#>

[CmdletBinding()]
param(
    [string]$VmName = "JainOS"
)

Write-Host "==============================================================================" -ForegroundColor Cyan
Write-Host "                 JAINOS — RUST MICRO OPERATING SYSTEM                          " -ForegroundColor Cyan
Write-Host "     Author: Vedav Suresh  |  USN: 25BTRCL280  |  Section: CSE AIML-E          " -ForegroundColor Yellow
Write-Host "==============================================================================" -ForegroundColor Cyan
Write-Host ""

# 1. Locate VBoxManage.exe
$vboxPaths = @(
    "$env:ProgramFiles\Oracle\VirtualBox\VBoxManage.exe",
    "${env:ProgramFiles(x86)}\Oracle\VirtualBox\VBoxManage.exe",
    "$env:VBOX_MSI_INSTALL_PATH\VBoxManage.exe",
    "$env:VBOX_INSTALL_PATH\VBoxManage.exe"
)

$vbox = $vboxPaths | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $vbox) {
    $vboxCmd = Get-Command VBoxManage.exe -ErrorAction SilentlyContinue
    if ($vboxCmd) { $vbox = $vboxCmd.Source }
}

if (-not $vbox) {
    Write-Error "Oracle VM VirtualBox is not detected. Please install VirtualBox or use QEMU."
    return
}

Write-Host "[OK] VirtualBox Engine: $vbox" -ForegroundColor Green

# 2. Locate ISO
$isoPaths = @(
    "$PSScriptRoot\build\JainOS.iso",
    "$PSScriptRoot\JainOS.iso",
    "\\wsl.localhost\Ubuntu-22.04\home\vedav\rust-micro-os\build\JainOS.iso"
)

$iso = $isoPaths | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $iso) {
    Write-Host "[*] Building JainOS.iso inside WSL..." -ForegroundColor Yellow
    wsl -d Ubuntu-22.04 -- bash -l -c "/home/vedav/rust-micro-os/build/build_jainos_iso.sh"
    $iso = $isoPaths | Where-Object { Test-Path $_ } | Select-Object -First 1
}

if (-not $iso) {
    Write-Error "Unable to locate or build JainOS.iso"
    return
}

$isoSize = (Get-Item $iso).Length
$isoSizeMB = [math]::Round($isoSize / 1MB, 2)
Write-Host "[OK] Bootable JainOS ISO: $iso ($isoSizeMB MB)" -ForegroundColor Green

# 3. Configure VM
Write-Host "[*] Checking VirtualBox VM '$VmName'..." -ForegroundColor Cyan
$vmExists = & $vbox list vms | Select-String -Pattern "`"$VmName`""

if ($vmExists) {
    Write-Host "[OK] Existing VM '$VmName' detected. Attaching media..." -ForegroundColor Green
    & $vbox storageattach $VmName --storagectl "IDE Controller" --port 0 --device 0 --type dvddrive --medium $iso 2>$null
} else {
    Write-Host "[*] Creating customized VM '$VmName'..." -ForegroundColor Yellow
    & $vbox createvm --name $VmName --ostype "Other_64" --register
    & $vbox modifyvm $VmName --memory 1024 --cpus 1 --vram 16 --graphicscontroller vmsvga
    & $vbox modifyvm $VmName --boot1 dvd --boot2 none --boot3 none --boot4 none
    & $vbox modifyvm $VmName --nic1 nat --nictype1 82540EM --macaddress1 525400123456
    & $vbox modifyvm $VmName --audio-enabled off
    & $vbox modifyvm $VmName --mouse ps2

    & $vbox storagectl $VmName --name "IDE Controller" --add ide
    & $vbox storageattach $VmName --storagectl "IDE Controller" --port 0 --device 0 --type dvddrive --medium $iso
    Write-Host "[OK] VM '$VmName' provisioned." -ForegroundColor Green
}

# 4. Start VM
Write-Host "==============================================================================" -ForegroundColor Cyan
Write-Host " Launching JainOS in VirtualBox..." -ForegroundColor Cyan
Write-Host "==============================================================================" -ForegroundColor Cyan
& $vbox startvm $VmName
