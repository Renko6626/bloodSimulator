@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\gpu-experiment.ps1" %*
set "experiment_exit=%errorlevel%"
echo.
echo Experiment exit code: %experiment_exit%
pause
exit /b %experiment_exit%
