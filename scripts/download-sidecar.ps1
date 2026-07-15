# Download sing-box 1.14.0 binary for Windows x64
# Author: TanXiang

$Version = "1.14.0-alpha.43"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$TargetDir = Join-Path $ScriptDir "..\src-tauri\sidecar-bin\windows-x64"
$ZipName = "sing-box-$Version-windows-amd64.zip"
$Url = "https://github.com/SagerNet/sing-box/releases/download/v$Version/$ZipName"
$TempZip = Join-Path $env:TEMP $ZipName

if (-not (Test-Path $TargetDir)) {
    New-Item -ItemType Directory -Path $TargetDir -Force | Out-Null
}

Write-Host "Downloading sing-box $Version from $Url ..."
Invoke-WebRequest -Uri $Url -OutFile $TempZip

Write-Host "Extracting sing-box executable..."
$ExtractPath = Join-Path $env:TEMP "sing-box-extract"
if (Test-Path $ExtractPath) { Remove-Item -Recurse -Force $ExtractPath }
Expand-Archive -Path $TempZip -DestinationPath $ExtractPath

$ExtractedExe = Get-ChildItem -Path $ExtractPath -Recurse -Filter "sing-box.exe" | Select-Object -First 1
if ($ExtractedExe) {
    $DestinationFile = Join-Path $TargetDir "sing-box-$Version.exe"
    Copy-Item -Path $ExtractedExe.FullName -Destination $DestinationFile -Force
    Write-Host "Successfully installed sing-box to $DestinationFile"
} else {
    Write-Error "Failed to find sing-box.exe in downloaded archive."
}

Remove-Item -Force $TempZip -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force $ExtractPath -ErrorAction SilentlyContinue
