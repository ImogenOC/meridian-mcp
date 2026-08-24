param([Parameter(Mandatory)][string]$Marker)

Start-Sleep -Seconds 2
Set-Content -LiteralPath $Marker -Value 'escaped'
