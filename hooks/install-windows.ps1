[CmdletBinding()]
param(
  [string]$ClaudeHome = (Join-Path $env:USERPROFILE ".claude")
)

# Claude Code on Windows runs hook `command` strings through Git Bash, so the reporter is the
# POSIX shell script (the same one macOS uses) and the command is `bash "<forward-slash path>"`.
# A backslash Windows path (e.g. a .cmd) is mangled by bash into "command not found".
$REPORTER  = "agent-monitor-report.sh"
$FARO_MARK = "agent-monitor-report"   # substring identifying a Faro hook group (any extension/form)
$EVENTS = @("SessionStart","UserPromptSubmit","PreToolUse","Notification","Stop","StopFailure","SessionEnd")

# Deep-convert ConvertFrom-Json output (PSCustomObject) into ordered hashtables/arrays
# so we can merge and re-serialize without losing or reordering existing keys.
function ConvertTo-HashtableDeep($obj) {
  if ($null -eq $obj) { return $null }
  if ($obj -is [System.Management.Automation.PSCustomObject]) {
    $h = [ordered]@{}
    foreach ($p in $obj.PSObject.Properties) { $h[$p.Name] = ConvertTo-HashtableDeep $p.Value }
    return $h
  }
  if ($obj -is [System.Collections.IDictionary]) {
    $h = [ordered]@{}
    foreach ($k in $obj.Keys) { $h[$k] = ConvertTo-HashtableDeep $obj[$k] }
    return $h
  }
  if ($obj -is [System.Collections.IEnumerable] -and $obj -isnot [string]) {
    return @(foreach ($i in $obj) { ConvertTo-HashtableDeep $i })
  }
  return $obj
}

$hooksDir     = Join-Path $ClaudeHome "hooks"
$dest         = Join-Path $hooksDir $REPORTER
$settingsPath = Join-Path $ClaudeHome "settings.json"
$srcReporter  = Join-Path $PSScriptRoot $REPORTER

# Bash-resolvable command: forward slashes + a `bash` prefix (so it does NOT depend on the
# exec bit, which a PowerShell copy cannot set). Quoted to tolerate spaces in the home path.
$destBash        = $dest -replace '\\','/'
$reporterCommand = 'bash "' + $destBash + '"'

if (-not (Test-Path $srcReporter)) {
  Write-Host "Faro: reporter not found next to installer: $srcReporter"
  exit 1
}

# 1. Load settings (or empty); abort on malformed JSON WITHOUT writing
$settings = [ordered]@{}
if (Test-Path $settingsPath) {
  $raw = Get-Content -Path $settingsPath -Raw
  if (-not [string]::IsNullOrWhiteSpace($raw)) {
    try { $parsed = $raw | ConvertFrom-Json -ErrorAction Stop }
    catch {
      Write-Host "Faro: settings.json is not valid JSON ($settingsPath). Aborting without changes."
      exit 1
    }
    $settings = ConvertTo-HashtableDeep $parsed
  }
}

# 2. Copy the reporter into <ClaudeHome>\hooks
New-Item -ItemType Directory -Force -Path $hooksDir | Out-Null
Copy-Item -Path $srcReporter -Destination $dest -Force

# 3. Ensure a hooks object
if (-not $settings.Contains("hooks") -or $null -eq $settings["hooks"] -or -not ($settings["hooks"] -is [System.Collections.IDictionary])) {
  $settings["hooks"] = [ordered]@{}
}
$hooks = $settings["hooks"]

# 4. For each event: drop any existing Faro group (idempotency / path refresh), keep
#    every non-Faro group, then append one fresh Faro group.
foreach ($evt in $EVENTS) {
  $kept = @()
  if ($hooks.Contains($evt) -and $hooks[$evt]) {
    foreach ($grp in @($hooks[$evt])) {
      $isFaro = $false
      if (($grp -is [System.Collections.IDictionary]) -and $grp.Contains("hooks") -and $grp["hooks"]) {
        foreach ($hh in @($grp["hooks"])) {
          if (($hh -is [System.Collections.IDictionary]) -and $hh.Contains("command") -and ("$($hh["command"])" -like "*$FARO_MARK*")) {
            $isFaro = $true
          }
        }
      }
      if (-not $isFaro) {
        if (($grp -is [System.Collections.IDictionary]) -and $grp.Contains("hooks")) { $grp["hooks"] = @($grp["hooks"]) }
        $kept += ,$grp
      }
    }
  }
  $faroGroup = [ordered]@{ hooks = @( ([ordered]@{ type = "command"; command = $reporterCommand }) ) }
  $kept += ,$faroGroup
  $hooks[$evt] = @($kept)
}
$settings["hooks"] = $hooks

# 5. Backup then write (UTF-8 no BOM, depth 10)
if (Test-Path $settingsPath) { Copy-Item -Path $settingsPath -Destination "$settingsPath.faro-bak" -Force }
$jsonOut = $settings | ConvertTo-Json -Depth 10
[System.IO.File]::WriteAllText($settingsPath, $jsonOut, (New-Object System.Text.UTF8Encoding($false)))

Write-Host "Faro: reporter installed to $dest"
Write-Host "Faro: registered $($EVENTS.Count) hook events in $settingsPath (backup: $settingsPath.faro-bak)"
Write-Host "Faro: restart/reload Claude Code so it re-reads settings.json."
exit 0
