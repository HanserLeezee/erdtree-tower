@echo off
chcp 65001 >nul
setlocal
echo ========================================================
echo   黄金树爬塔 - 启动崩溃自动诊断工具
echo ========================================================
echo 本工具只读采集信息并生成 startup-diagnostic.txt，不会修改存档。
echo.
set /p DO_DIRECT=是否顺便测试“绕过加载器直接裸跑 eldenring.exe 12秒”？(Y/N，推荐选 Y): 
if /i "%DO_DIRECT%"=="Y" (
    "%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Diagnose-Startup.ps1" -TestDirectLaunch
) else (
    "%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Diagnose-Startup.ps1"
)
echo.
pause
