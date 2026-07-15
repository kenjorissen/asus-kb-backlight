[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path -LiteralPath $cargo)) {
    throw "Cargo was not found at $cargo"
}

& $cargo test --all-targets
if ($LASTEXITCODE -ne 0) { throw "cargo test failed with exit code $LASTEXITCODE" }

& $cargo build --release --bins
if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }

$isccCommand = Get-Command ISCC.exe -ErrorAction SilentlyContinue
$isccPath = if ($isccCommand) {
    $isccCommand.Source
} else {
    $candidates = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
    )
    $found = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $found) {
        throw 'Inno Setup 6 is required. Install it with: winget install JRSoftware.InnoSetup'
    }
    $found
}

& $isccPath (Join-Path $root 'installer\asus-kbdlight.iss')
if ($LASTEXITCODE -ne 0) { throw "ISCC failed with exit code $LASTEXITCODE" }

Get-ChildItem -LiteralPath (Join-Path $root 'dist') -Filter 'AsusKbdLightSetup-*.exe' |
    Select-Object FullName, Length, LastWriteTime
