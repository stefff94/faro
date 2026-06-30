@echo off
setlocal
if "%FARO_BROKER_URL%"=="" (set "U=http://127.0.0.1:8765/event") else (set "U=%FARO_BROKER_URL%")
curl.exe -s -m 1 -X POST "%U%" -H "Content-Type: application/json" --data-binary @- >nul 2>&1
exit /b 0
