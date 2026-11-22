<#
.SYNOPSIS
    JainOS Windows Host Wi-Fi Helper Service (PowerShell edition)
.DESCRIPTION
    Queries the Windows laptop Wi-Fi interfaces and networks, serving them to JainOS guest over TCP 19820.
#>

$port = 19820
$listener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Any, $port)
$listener.Start()
Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host "       JainOS Windows Host Wi-Fi Helper Service (Port $port)     " -ForegroundColor Yellow
Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host "[+] Listening for JainOS guest requests on 0.0.0.0:$port..." -ForegroundColor Green

try {
    while ($true) {
        $client = $listener.AcceptTcpClient()
        $stream = $client.GetStream()
        $reader = New-Object System.IO.StreamReader($stream)
        $writer = New-Object System.IO.StreamWriter($stream)

        $line = $reader.ReadLine()
        Write-Host "[+] Request from $($client.Client.RemoteEndPoint): $line" -ForegroundColor DarkCyan

        # Query host Wi-Fi
        $ifaces = netsh wlan show interfaces
        $connectedSsid = "Disconnected"
        $signal = "0%"
        $state = "DISCONNECTED"
        $adapter = "Unknown"
        $radio = "802.11ax"

        foreach ($l in $ifaces) {
            $trimmed = $l.Trim()
            if ($trimmed -match "^Description\s*:\s*(.*)$") { $adapter = $matches[1] }
            elseif ($trimmed -match "^State\s*:\s*(.*)$") {
                if ($matches[1] -match "connected") { $state = "CONNECTED" }
            }
            elseif ($trimmed -match "^SSID\s*:\s*(.*)$") { $connectedSsid = $matches[1] }
            elseif ($trimmed -match "^Signal\s*:\s*(.*)$") { $signal = $matches[1] }
            elseif ($trimmed -match "^Radio type\s*:\s*(.*)$") { $radio = $matches[1] }
        }

        # Query available networks
        $nets = netsh wlan show networks
        $avail = [System.Collections.Generic.List[string]]::new()
        foreach ($l in $nets) {
            $trimmed = $l.Trim()
            if ($trimmed -match "^SSID \d+\s*:\s*(.*)$") {
                $sName = $matches[1].Trim()
                if ($sName -and (-not $avail.Contains($sName))) {
                    $avail.Add($sName)
                }
            }
        }
        $availStr = ($avail -join ",")

        $response = "RIFTNET_WIFI_V1`n" +
                    "STATE:$state`n" +
                    "SSID:$connectedSsid`n" +
                    "SIGNAL:$signal`n" +
                    "ADAPTER:$adapter`n" +
                    "RADIO:$radio`n" +
                    "AVAILABLE:$availStr`n" +
                    "END`n"

        $writer.Write($response)
        $writer.Flush()
        $client.Close()
    }
}
finally {
    $listener.Stop()
}
