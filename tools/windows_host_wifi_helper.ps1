# JainOS Windows Host Wi-Fi Helper Service (PowerShell edition v0.3.2)
# Author: Vedav Suresh (USN: 25BTRCL280)
# Project: JainOS — Rust Micro Operating System

$port = 19820
$listener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Any, $port)
$listener.Start()

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "      JainOS Host Wi-Fi Helper Service (v0.3.2 - PowerShell)   " -ForegroundColor Green
Write-Host "      Listening on 0.0.0.0:$port (Gateway 10.0.2.2)            " -ForegroundColor Yellow
Write-Host "================================================================" -ForegroundColor Cyan

try {
    while ($true) {
        $client = $listener.AcceptTcpClient()
        $stream = $client.GetStream()
        $reader = New-Object System.IO.StreamReader($stream)
        $writer = New-Object System.IO.StreamWriter($stream)
        $writer.AutoFlush = $true

        $request = $reader.ReadLine()
        Write-Host "[+] Request: $request" -ForegroundColor Gray

        if ($request -match "^CONNECT_KEY (.+)") {
            $payload = $matches[1].Trim()
            $parts = $payload -split "\|", 2
            $ssid = $parts[0].Trim()
            $pass = if ($parts.Length -gt 1) { $parts[1].Trim() } else { $null }
            
            if ($pass) {
                # Create profile with password
                $xml = @"
<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
    <name>$ssid</name>
    <SSIDConfig>
        <SSID><name>$ssid</name></SSID>
    </SSIDConfig>
    <connectionType>ESS</connectionType>
    <connectionMode>manual</connectionMode>
    <MSM>
        <security>
            <authEncryption>
                <authentication>WPA2PSK</authentication>
                <encryption>AES</encryption>
                <useOneX>false</useOneX>
            </authEncryption>
            <sharedKey>
                <keyType>passPhrase</keyType>
                <protected>false</protected>
                <keyMaterial>$pass</keyMaterial>
            </sharedKey>
        </security>
    </MSM>
</WLANProfile>
"@
                $tmpPath = [System.IO.Path]::GetTempFileName() + ".xml"
                [System.IO.File]::WriteAllText($tmpPath, $xml)
                netsh wlan add profile filename="$tmpPath" | Out-Null
                Remove-Item -Force $tmpPath -ErrorAction SilentlyContinue
            }
            netsh wlan connect name="$ssid" | Out-Null
            Start-Sleep -Seconds 1
            $writer.WriteLine("OK:CONNECTED")
        } elseif ($request -match "^CONNECT (.+)") {
            $ssid = $matches[1].Trim()
            netsh wlan connect name="$ssid" | Out-Null
            Start-Sleep -Seconds 1
            $writer.WriteLine("OK:CONNECTED")
        } elseif ($request -match "^DISCONNECT") {
            netsh wlan disconnect | Out-Null
            Start-Sleep -Seconds 1
            $writer.WriteLine("OK:DISCONNECTED")
        } elseif ($request -match "^WIFI_OFF") {
            netsh wlan disconnect | Out-Null
            $writer.WriteLine("OK:TOGGLED")
        } elseif ($request -match "^WIFI_ON") {
            $writer.WriteLine("OK:TOGGLED")
        } else {
            # Query interfaces
            $ifaceOut = netsh wlan show interfaces
            $adapter = "MediaTek Wi-Fi 6 MT7921 Wireless LAN Card"
            $state = "DISCONNECTED"
            $connectedSsid = "None"
            $signal = "0%"
            $radio = "802.11ax"
            $auth = "None"

            foreach ($line in ($ifaceOut -split "`n")) {
                $trimmed = $line.Trim()
                if ($trimmed -match "^Description\s*:\s*(.+)") { $adapter = $matches[1].Trim() }
                if ($trimmed -match "^State\s*:\s*(.+)") {
                    $stVal = $matches[1].Trim().ToLower()
                    if ($stVal -eq "connected") {
                        $state = "CONNECTED"
                    } else {
                        $state = "DISCONNECTED"
                        $connectedSsid = "None"
                        $signal = "0%"
                    }
                }
                if ($trimmed -match "^SSID\s*:\s*(.+)" -and -not ($trimmed -match "^SSID 1")) {
                    $connectedSsid = $matches[1].Trim()
                }
                if ($trimmed -match "^Signal\s*:\s*(.+)") { $signal = $matches[1].Trim() }
                if ($trimmed -match "^Radio type\s*:\s*(.+)") { $radio = $matches[1].Trim() }
                if ($trimmed -match "^Authentication\s*:\s*(.+)") { $auth = $matches[1].Trim() }
            }

            # Query visible networks
            $netOut = netsh wlan show networks mode=bssid
            $networks = @()
            $curSsid = $null
            $curAuth = "Open"
            $curSignal = "0%"

            foreach ($line in ($netOut -split "`n")) {
                $trimmed = $line.Trim()
                if ($trimmed -match "^SSID \d+\s*:\s*(.+)") {
                    if ($curSsid) {
                        $isConn = if ($curSsid -eq $connectedSsid -and $state -eq "CONNECTED") { "YES" } else { "NO" }
                        $networks += "$curSsid|$curSignal|$curAuth|$isConn|YES"
                    }
                    $curSsid = $matches[1].Trim()
                    $curAuth = "Open"
                    $curSignal = "0%"
                }
                if ($trimmed -match "^Authentication\s*:\s*(.+)") { $curAuth = $matches[1].Trim() }
                if ($trimmed -match "^Signal\s*:\s*(.+)") { $curSignal = $matches[1].Trim() }
            }
            if ($curSsid) {
                $isConn = if ($curSsid -eq $connectedSsid -and $state -eq "CONNECTED") { "YES" } else { "NO" }
                $networks += "$curSsid|$curSignal|$curAuth|$isConn|YES"
            }

            $response = "JAINOS_WIFI_V2`nSTATE:$state`nCONNECTED_SSID:$connectedSsid`nSIGNAL:$signal`nADAPTER:$adapter`nRADIO:$radio`nAUTH:$auth`nNETWORKS:`n"
            foreach ($net in $networks) {
                $response += "$net`n"
            }
            $response += "END`n"
            $writer.Write($response)
        }

        $client.Close()
    }
} finally {
    $listener.Stop()
}
