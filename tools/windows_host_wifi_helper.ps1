# JainOS Windows Host Wi-Fi Helper Service (PowerShell edition v0.3.1)
# Author: Vedav Suresh (USN: 25BTRCL280)
# Project: JainOS — Rust Micro Operating System

$port = 19820
$listener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Any, $port)
$listener.Start()

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "      JainOS Host Wi-Fi Helper Service (v0.3.1 - PowerShell)   " -ForegroundColor Green
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

        if ($request -match "^CONNECT (.+)") {
            $ssid = $matches[1].Trim()
            netsh wlan connect name="$ssid" | Out-Null
            Start-Sleep -Seconds 1
            $writer.WriteLine("OK:CONNECTED")
        } elseif ($request -match "^DISCONNECT") {
            netsh wlan disconnect | Out-Null
            Start-Sleep -Seconds 1
            $writer.WriteLine("OK:DISCONNECTED")
        } else {
            # Query interfaces
            $ifaceOut = netsh wlan show interfaces
            $adapter = "Wi-Fi"
            $state = "DISCONNECTED"
            $connectedSsid = "None"
            $signal = "0%"
            $radio = "802.11ax"
            $auth = "None"

            foreach ($line in ($ifaceOut -split "`n")) {
                $trimmed = $line.Trim()
                if ($trimmed -match "^Description\s*:\s*(.+)") { $adapter = $matches[1].Trim() }
                if ($trimmed -match "^State\s*:\s*(.+)") {
                    if ($matches[1] -match "connected") { $state = "CONNECTED" }
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
