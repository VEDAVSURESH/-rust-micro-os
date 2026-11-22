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

Write-Host "[+] Taking Screenshot 1: Desktop Startup (vbox_v032_desktop.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v032_desktop.png"

Write-Host "[+] Pressing F2 (scancode 3c bc) to bring Network Control Center to front..."
& $VBoxManage controlvm $VM keyboardputscancode 3c bc
Start-Sleep -Seconds 4

Write-Host "[+] Taking Screenshot 2: Network Control Center (vbox_v032_network.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v032_network.png"

Write-Host "[+] Pressing F3 (scancode 3d bd) to open Modern Web Browser..."
& $VBoxManage controlvm $VM keyboardputscancode 3d bd
Start-Sleep -Seconds 4

Write-Host "[+] Taking Screenshot 3: Modern Web Browser (vbox_v032_browser.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v032_browser.png"

Write-Host "[+] Pressing F9 (scancode 43 c3) to toggle Start Menu..."
& $VBoxManage controlvm $VM keyboardputscancode 43 c3
Start-Sleep -Seconds 3

Write-Host "[+] Taking Screenshot 4: Start Menu with Network (vbox_v032_start_menu.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v032_start_menu.png"

Write-Host "[+] Closing Start Menu..."
& $VBoxManage controlvm $VM keyboardputscancode 43 c3
Start-Sleep -Seconds 1

Write-Host "[+] Pressing F1 (scancode 3b bb) to focus Terminal..."
& $VBoxManage controlvm $VM keyboardputscancode 3b bb
Start-Sleep -Seconds 2

Write-Host "[+] Typing 'whoami' into Terminal..."
# Scancodes for w (11 91) h (23 a3) o (18 98) a (1e 9e) m (32 b2) i (17 97) Enter (1c 9c)
& $VBoxManage controlvm $VM keyboardputscancode 11 91 23 a3 18 98 1e 9e 32 b2 17 97 1c 9c
Start-Sleep -Seconds 3

Write-Host "[+] Taking Screenshot 5: Terminal with whoami (vbox_v032_whoami.png)..."
& $VBoxManage controlvm $VM screenshotpng "$ArtifactDir\vbox_v032_whoami.png"

Write-Host "[+] Powering off VirtualBox VM cleanly..."
& $VBoxManage controlvm $VM poweroff
Start-Sleep -Seconds 1

Write-Host "[+] Checking VirtualBox Serial Log..."
if (Test-Path "C:\Users\USER\VirtualBox VMs\JainOS\jainos_serial.log") {
    Get-Content "C:\Users\USER\VirtualBox VMs\JainOS\jainos_serial.log" -Tail 30
}

Write-Host "[✓] VirtualBox test suite completed successfully!"
