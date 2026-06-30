# Plain assert-based test for install-windows.ps1 (no framework). Run: powershell -File hooks\install-windows.tests.ps1
$ErrorActionPreference = "Stop"
$installer = Join-Path $PSScriptRoot "install-windows.ps1"
$script:fail = 0
function Check($cond, $msg) {
  if ($cond) { Write-Host "  ok:   $msg" } else { Write-Host "  FAIL: $msg"; $script:fail++ }
}
function New-TempHome {
  $t = Join-Path ([System.IO.Path]::GetTempPath()) ("faro-test-" + [System.Guid]::NewGuid().ToString("N"))
  New-Item -ItemType Directory -Force -Path $t | Out-Null
  return $t
}
function Run-Installer($claudeHome) {
  & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $installer -ClaudeHome $claudeHome | Out-Null
  return $LASTEXITCODE
}
$events = @("SessionStart","UserPromptSubmit","PreToolUse","Notification","Stop","StopFailure","SessionEnd")

Write-Host "Test 1: preserves existing keys, registers 7 events, copies reporter, makes backup"
$h1 = New-TempHome
$seed = @{
  model = "sonnet"
  enabledPlugins = @{ foo = $true }
  hooks = @{ PreToolUse = @( @{ hooks = @( @{ type = "command"; command = "C:/other/hook.cmd" } ) } ) }
}
[System.IO.File]::WriteAllText((Join-Path $h1 "settings.json"), ($seed | ConvertTo-Json -Depth 10), (New-Object System.Text.UTF8Encoding($false)))
$code = Run-Installer $h1
Check ($code -eq 0) "installer exits 0"
$s = Get-Content (Join-Path $h1 "settings.json") -Raw | ConvertFrom-Json
Check ($s.model -eq "sonnet") "existing key 'model' preserved"
Check ($s.enabledPlugins.foo -eq $true) "existing key 'enabledPlugins' preserved"
foreach ($e in $events) {
  $arr = @($s.hooks.$e)
  $hasFaro = (@($arr | Where-Object { @($_.hooks.command) -like "*agent-monitor-report.sh*" }).Count -ge 1)
  Check $hasFaro "event $e registers the Faro reporter"
}
$pre = @($s.hooks.PreToolUse)
# NB: this only checks the command survives; Test 4 is what guards that its `hooks` stays a JSON array.
Check ((@($pre | Where-Object { @($_.hooks.command) -like "*other/hook.cmd*" }).Count) -eq 1) "PreToolUse keeps the pre-existing non-Faro hook"
Check (Test-Path (Join-Path $h1 "hooks\agent-monitor-report.sh")) "reporter copied into ClaudeHome\hooks"
Check (Test-Path (Join-Path $h1 "settings.json.faro-bak")) "backup created"
# Regression guard for the Git-Bash execution bug: the command must be the bash "<fwd path>"
# form (forward slashes, no backslashes); a backslash path is mangled by bash to "command not found".
$faroCmd = @(@($s.hooks.SessionStart).hooks.command | Where-Object { "$_" -like "*agent-monitor-report.sh*" })[0]
Check ($faroCmd -like 'bash "*"') 'Faro command uses the bash "..." form'
Check ($faroCmd -notlike '*\*') 'Faro command has no backslashes (bash-safe)'
Check ($faroCmd -like '*/agent-monitor-report.sh"') 'Faro command targets the .sh via forward slashes'
# Guard the Faro group's OWN single-element `hooks` against PS 5.1 array->object collapse (raw-text).
$raw1 = Get-Content (Join-Path $h1 "settings.json") -Raw
Check ($raw1 -match '"hooks"\s*:\s*\[\s*\{[^\[\]]*agent-monitor-report\.sh') "Faro group 'hooks' stays a JSON array"

Write-Host "Test 2: idempotent re-run (no duplicate Faro groups)"
Run-Installer $h1 | Out-Null
$s2 = Get-Content (Join-Path $h1 "settings.json") -Raw | ConvertFrom-Json
$preFaro = (@(@($s2.hooks.PreToolUse) | Where-Object { @($_.hooks.command) -like "*agent-monitor-report.sh*" }).Count)
Check ($preFaro -eq 1) "PreToolUse has exactly one Faro group after re-run"
Check ((@($s2.hooks.SessionStart).Count) -eq 1) "SessionStart has exactly one group after re-run"
Check ((@(@($s2.hooks.PreToolUse) | Where-Object { @($_.hooks.command) -like "*other/hook.cmd*" }).Count) -eq 1) "PreToolUse keeps exactly one non-Faro group after re-run"

Write-Host "Test 3: aborts on malformed settings.json without overwriting it"
$h3 = New-TempHome
$bad = "{ this is not json"
[System.IO.File]::WriteAllText((Join-Path $h3 "settings.json"), $bad, (New-Object System.Text.UTF8Encoding($false)))
$code3 = Run-Installer $h3
Check ($code3 -ne 0) "installer exits non-zero on malformed settings"
Check (((Get-Content (Join-Path $h3 "settings.json") -Raw).Trim()) -eq $bad) "malformed settings.json left unchanged"
Check (-not (Test-Path (Join-Path $h3 "hooks\agent-monitor-report.sh"))) "no reporter copied on malformed abort (true no-op)"
Check (-not (Test-Path (Join-Path $h3 "settings.json.faro-bak"))) "no backup written on malformed abort"

Write-Host "Test 4: a pre-existing non-Faro hook keeps its array structure"
$h4 = New-TempHome
$seed4 = @{ hooks = @{ PreToolUse = @( @{ matcher = "Bash"; hooks = @( @{ type = "command"; command = "C:/other/hook.cmd" } ) } ) } }
[System.IO.File]::WriteAllText((Join-Path $h4 "settings.json"), ($seed4 | ConvertTo-Json -Depth 10), (New-Object System.Text.UTF8Encoding($false)))
Run-Installer $h4 | Out-Null
$raw4 = (Get-Content (Join-Path $h4 "settings.json") -Raw)
Check ($raw4 -match '"hooks"\s*:\s*\[\s*\{[^\[\]]*other/hook\.cmd') "pre-existing non-Faro hook 'hooks' stays a JSON array"
Remove-Item -Recurse -Force $h4 -ErrorAction SilentlyContinue

Write-Host "Test 5: migrates an old .cmd Faro registration to the bash .sh form"
$h5 = New-TempHome
$old5 = @{ hooks = @{ SessionStart = @( @{ hooks = @( @{ type = "command"; command = "C:\Users\x\.claude\hooks\agent-monitor-report.cmd" } ) } ) } }
[System.IO.File]::WriteAllText((Join-Path $h5 "settings.json"), ($old5 | ConvertTo-Json -Depth 10), (New-Object System.Text.UTF8Encoding($false)))
Run-Installer $h5 | Out-Null
$s5 = Get-Content (Join-Path $h5 "settings.json") -Raw | ConvertFrom-Json
$ss5 = @($s5.hooks.SessionStart)
Check ($ss5.Count -eq 1) "old .cmd Faro group removed (one SessionStart group remains)"
$cmds5 = @($ss5.hooks.command)
Check ((@($cmds5 | Where-Object { "$_" -like "*.cmd*" }).Count) -eq 0) "no .cmd command remains after migration"
Check ((@($cmds5 | Where-Object { "$_" -like 'bash "*agent-monitor-report.sh"' }).Count) -eq 1) "registered the bash .sh command"
Remove-Item -Recurse -Force $h5 -ErrorAction SilentlyContinue

Remove-Item -Recurse -Force $h1, $h3 -ErrorAction SilentlyContinue
if ($script:fail -gt 0) { Write-Host "`n$($script:fail) assertion(s) FAILED"; exit 1 }
Write-Host "`nAll installer tests passed"; exit 0
