# Baut Klassenwerk für Windows: Programm, portables Zip und Installer (dist\).
# Voraussetzungen: Rust (https://rustup.rs) und für den Installer Inno Setup 6
# (https://jrsoftware.org/isdl.php). Aufruf im Projektordner:
#   powershell -ExecutionPolicy Bypass -File packaging\windows\build-windows.ps1
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\..\.."
Push-Location "$root\app"
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build ist fehlgeschlagen" }
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"').Matches[0].Groups[1].Value
Pop-Location
$exe = "$root\app\target\release\Klassenwerk.exe"
New-Item -ItemType Directory -Force "$root\dist" | Out-Null

# portables Zip
$tmp = Join-Path $env:TEMP "Klassenwerk-$version"
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $tmp | Out-Null
Copy-Item $exe $tmp
Copy-Item "$root\LICENSE" "$tmp\LICENSE.txt"
Copy-Item "$root\THIRD_PARTY_LICENSES.txt" $tmp
$zip = "$root\dist\Klassenwerk-$version-windows-portable.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path $tmp -DestinationPath $zip
Write-Host "Fertig: $zip"

# Installer
$iscc = Get-Command iscc -ErrorAction SilentlyContinue
if ($iscc) { $iscc = $iscc.Source } else { $iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" }
if (Test-Path $iscc) {
    & $iscc "/DAppVersion=$version" "/DSourceExe=$exe" "$root\packaging\windows\klassenwerk.iss"
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup ist fehlgeschlagen" }
    Write-Host "Fertig: $root\dist\Klassenwerk-$version-windows-setup.exe"
} else {
    Write-Host "Inno Setup 6 nicht gefunden – nur das portable Zip wurde erstellt."
}
