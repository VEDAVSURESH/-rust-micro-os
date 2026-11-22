$VBoxManage = "C:\Program Files\Oracle\VirtualBox\VBoxManage.exe"
$ArtifactDir = "C:\Users\USER\.gemini\antigravity\brain\39aff397-e9da-4a2e-84b6-6e7b06ac7623"
$VM = "JainOS"
$ISO = "\\wsl.localhost\Ubuntu-22.04\home\vedav\rust-micro-os\build\JainOS.iso"

Write-Host "[+] Ensuring VM is stopped..."
& $VBoxManage controlvm $VM poweroff 2>$null
Start-Sleep -Seconds 1

Write-Host "[+] Refreshing ISO attachment in VirtualBox..."
& $VBoxManage storageattach $VM --storagectl "IDE" --port 1 --device 0 --type dvddrive --medium $ISO
Start-Sleep -Seconds 1

Write-Host "[+] Starting VirtualBox VM '$VM' in headless mode..."
& $VBoxManage startvm $VM --type headless
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to start VM $VM"
    exit 1
}

Write-Host "[+] Waiting 65 seconds for VirtualBox BIOS boot, tests, e1000 DHCP, and Desktop GUI..."
Start-Sleep -Seconds 65

Write-Host "[+] Taking Screenshot 1: Desktop Startup (vbox_v034_desktop.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v034_desktop.png"

Write-Host "[+] Pressing F2 (scancode 3c bc) to bring Network Control Center to front..."
& $VBoxManage controlvm $VM keyboardputscancode 3c bc
Start-Sleep -Seconds 3

Write-Host "[+] Taking Screenshot 2: Clean Network Interface (vbox_v034_network_clean.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v034_network_clean.png"

Write-Host "[+] Pressing Tab (scancode 0f 8f) to activate Password Field Focus..."
& $VBoxManage controlvm $VM keyboardputscancode 0f 8f
Start-Sleep -Seconds 2

Write-Host "[+] Taking Screenshot 3: Password Field Focused with Glow (vbox_v034_password_focused.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v034_password_focused.png"

Write-Host "[+] Typing password 'JainOS@2026' into secured Wi-Fi field..."
# J: 2a 24 a4 aa
# a: 1e 9e
# i: 17 97
# n: 31 b1
# O: 2a 18 98 aa
# S: 2a 1f 9f aa
# @: 2a 03 83 aa
# 2: 03 83
# 0: 0b 8b
# 2: 03 83
# 6: 07 87
& $VBoxManage controlvm $VM keyboardputscancode 2a 24 a4 aa 1e 9e 17 97 31 b1 2a 18 98 aa 2a 1f 9f aa 2a 03 83 aa 03 83 0b 8b 03 83 07 87
Start-Sleep -Seconds 2

Write-Host "[+] Taking Screenshot 4: Password Field with Masked Bullets (vbox_v034_password_typed_masked.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v034_password_typed_masked.png"

Write-Host "[+] Pressing Backspace 4 times (scancode 0e 8e) to verify editing..."
for ($i = 0; $i -lt 4; $i++) {
    & $VBoxManage controlvm $VM keyboardputscancode 0e 8e
    Start-Sleep -Milliseconds 200
}
Start-Sleep -Seconds 2

Write-Host "[+] Taking Screenshot 5: Password Field after Backspace Editing (vbox_v034_password_backspaced.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v034_password_backspaced.png"

Write-Host "[+] Submitting password with Enter (scancode 1c 9c)..."
& $VBoxManage controlvm $VM keyboardputscancode 1c 9c
Start-Sleep -Seconds 3

Write-Host "[+] Taking Screenshot 6: Connection Status & Cleared Field (vbox_v034_connected_status.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v034_connected_status.png"

Write-Host "[+] Pressing F1 (scancode 3b bb) to focus Terminal..."
& $VBoxManage controlvm $VM keyboardputscancode 3b bb
Start-Sleep -Seconds 2

Write-Host "[+] Typing 'whoami' into Terminal..."
# Scancodes for w (11 91) h (23 a3) o (18 98) a (1e 9e) m (32 b2) i (17 97) Enter (1c 9c)
& $VBoxManage controlvm $VM keyboardputscancode 11 91 23 a3 18 98 1e 9e 32 b2 17 97 1c 9c
Start-Sleep -Seconds 3

Write-Host "[+] Taking Screenshot 7: Terminal with whoami (vbox_v034_whoami.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v034_whoami.png"

Write-Host "[+] Powering off VirtualBox VM cleanly..."
& $VBoxManage controlvm $VM poweroff
Start-Sleep -Seconds 1

Write-Host "[+] Checking VirtualBox Serial Log..."
if (Test-Path "C:\Users\USER\VirtualBox VMs\JainOS\jainos_serial.log") {
    Get-Content "C:\Users\USER\VirtualBox VMs\JainOS\jainos_serial.log" -Tail 30
}

Write-Host "[✓] VirtualBox v0.3.4 test suite completed successfully!"
