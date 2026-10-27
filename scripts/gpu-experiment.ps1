param([string]$Python = '', [string]$OutputDirectory = '')
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $repo
$env:PYTHONUTF8 = '1'
if (-not $Python) {
    $bundled = Join-Path $env:USERPROFILE '.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe'
    if (Test-Path -LiteralPath $bundled) { $Python = $bundled }
    else { $Python = 'python' }
}
& $Python --version
if ($LASTEXITCODE -ne 0) { throw 'Python 3 is required. Pass -Python with its executable path.' }
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $repo ('target\gpu-experiments\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
}
if (Test-Path -LiteralPath $OutputDirectory) { throw "Output already exists: $OutputDirectory" }
$out = New-Item -ItemType Directory -Path $OutputDirectory
$lab = Join-Path $repo 'docs\research\experiments\2026-10-27-gpu-stokes'
$target = Join-Path $repo 'target\gpu-lab'
Start-Transcript -Path (Join-Path $out.FullName 'session.log') | Out-Null
try {
    $hardware = [ordered]@{
        recorded_at = (Get-Date -Format o)
        cpu = @(Get-CimInstance Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors)
        gpu = @(Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion)
        memory_bytes = (Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory
        git_commit = (git rev-parse HEAD)
        git_status = @(git status --short)
    }
    $hardware | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $out.FullName 'hardware.json')
    & cargo test --release --locked --manifest-path "$lab\lab\Cargo.toml" --target-dir $target -j 4
    if ($LASTEXITCODE -ne 0) { throw 'GPU lab tests failed.' }
    & cargo build --release --locked --manifest-path "$lab\lab\Cargo.toml" --target-dir $target -j 4
    if ($LASTEXITCODE -ne 0) { throw 'GPU lab build failed.' }
    $binary = Join-Path $target 'release\blood-gpu-stokes-lab.exe'
    & $binary probe --out (Join-Path $out.FullName 'probe.json')
    if ($LASTEXITCODE -ne 0) { throw 'Physical GPU probe failed. See probe output.' }
    & $binary smoke --out (Join-Path $out.FullName 'smoke.json')
    if ($LASTEXITCODE -ne 0) { throw 'GPU numerical smoke check failed.' }
    & $Python "$lab\prepare.py" --out (Join-Path $out.FullName 'data') --target-dir (Join-Path $repo 'target\gpu-capture')
    if ($LASTEXITCODE -ne 0) { throw 'Matrix capture failed. See data build/capture logs.' }
    & $Python "$lab\cloud.py" --data (Join-Path $out.FullName 'data') --out (Join-Path $out.FullName 'results') --target-dir $target
    if ($LASTEXITCODE -ne 0) { throw 'Experiment recorded a failure or nonconvergence. Inspect results\summary.json and raw logs.' }
    Write-Host "Experiment passed: $($out.FullName)"
} finally {
    Write-Host "Experiment files: $($out.FullName)"
    Stop-Transcript | Out-Null
}
