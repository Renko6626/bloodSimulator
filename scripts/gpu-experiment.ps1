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
$code = 1
$hardware = [ordered]@{
    recorded_at = (Get-Date -Format o)
    cpu = @(Get-CimInstance Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors)
    gpu = @(Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion)
    memory_bytes = (Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory
    git_commit = (git rev-parse HEAD)
    git_status = @(git status --short)
}
try {
    & $Python (Join-Path $PSScriptRoot 'fluid-experiment.py') --out $OutputDirectory
    $code = $LASTEXITCODE
    if ($code -ne 0) { Write-Host 'Experiment failed or did not converge; inspect summary.json and raw logs.' }
} finally {
    if (Test-Path -LiteralPath $OutputDirectory) {
        $hardware | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $OutputDirectory 'hardware.json')
    }
    Write-Host "Experiment files: $OutputDirectory"
}
exit $code
