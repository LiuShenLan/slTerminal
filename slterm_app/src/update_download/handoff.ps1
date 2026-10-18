# 独立安装 helper(09 篇 M10.5:两阶段 handoff 的 helper 侧)。
# 在事务目录内运行,位于被替换的安装目录之外。
# 应用只有在 ready.json 之后写 commit.json 才授予安装权威;否则本脚本
# 回滚路径收尾并退出,绝不自行安装。
param([Parameter(Mandatory = $true)][string]$PlanPath)
$ErrorActionPreference = 'Stop'
$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$transaction = Split-Path -Parent $PlanPath
$utf8 = [System.Text.UTF8Encoding]::new($false)
$handles = @()
$guard = $null
$installerGuard = $null
$committed = $false
$originalDigest = $null

# 原子写事务状态文件:临时文件 + Replace/Move,读者永远看不到半写 JSON。
function Write-State([string]$Name, $Value) {
    $destination = Join-Path $transaction $Name
    $temporary = "$destination.$PID.tmp"
    $bytes = $utf8.GetBytes(($Value | ConvertTo-Json -Depth 12 -Compress))
    $stream = [System.IO.FileStream]::new($temporary, [System.IO.FileMode]::CreateNew,
        [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
    try { $stream.Write($bytes, 0, $bytes.Length); $stream.Flush($true) }
    finally { $stream.Dispose() }
    if (Test-Path -LiteralPath $destination) {
        [System.IO.File]::Replace($temporary, $destination, $null)
    } else { [System.IO.File]::Move($temporary, $destination) }
}

function Same-Path([string]$First, [string]$Second) {
    return [string]::Equals([System.IO.Path]::GetFullPath($First).TrimEnd('\'),
        [System.IO.Path]::GetFullPath($Second).TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase)
}

# 版本回读:安装器成功 ≠ 装进去的是新版本,必须读回 `slterm --version`。
function Read-Version([string]$Executable) {
    $start = [System.Diagnostics.ProcessStartInfo]::new($Executable, '--version')
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $probe = [System.Diagnostics.Process]::Start($start)
    try {
        $output = $probe.StandardOutput.ReadToEndAsync()
        $errors = $probe.StandardError.ReadToEndAsync()
        if (-not $probe.WaitForExit(10000)) {
            $probe.Kill()
            $probe.WaitForExit()
            throw 'Application version verification timed out'
        }
        if ($probe.ExitCode -ne 0) { throw 'Application version verification failed' }
        return $output.Result.Trim()
    } finally { $probe.Dispose() }
}

# 重启新进程并注入一次性恢复票 + 配置目录(单名,双名别名层不迁)。
function Launch-Workspace {
    $env:SLTERM_UPDATE_RESTORE = $PlanPath
    $env:SLTERM_CONFIG_DIR = $plan.config_directory
    Start-Process -FilePath $exe -WorkingDirectory $installation | Out-Null
}

# 同安装基的其他进程必须先退出:更新只对自己准备好的参与者授权。
function Check-UnpreparedProcesses {
    foreach ($other in [System.Diagnostics.Process]::GetProcessesByName([System.IO.Path]::GetFileNameWithoutExtension($exe))) {
        try {
            if ((Same-Path $other.MainModule.FileName $exe) -and
                -not (@($plan.participants | Where-Object { [int]$_.pid -eq $other.Id }).Count)) {
                throw 'Another process from this installation is not prepared for update'
            }
        } finally { $other.Dispose() }
    }
}

# 新 helper 可能仍在收尾;安装器复制文件前等它们释放句柄。
# 只轮询少量已知文件,绝不按映像名杀进程。
function Wait-RuntimeHelperFiles {
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    $helpers = @('runtime\slterm-hook.exe', 'slterm-hook.exe')
    while ($true) {
        $busy = $null
        foreach ($relative in $helpers) {
            $path = Join-Path $installation $relative
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
            try {
                $probe = [System.IO.FileStream]::new($path, [System.IO.FileMode]::Open,
                    [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
                $probe.Dispose()
            } catch { $busy = $_.Exception }
        }
        if (-not $busy) { return }
        if ([DateTime]::UtcNow -ge $deadline) { throw $busy }
        Start-Sleep -Milliseconds 100
    }
}

try {
    if ((Get-Item -LiteralPath $PlanPath).Length -gt 1048576) { throw 'Update plan exceeds limit' }
    $plan = Get-Content -LiteralPath $PlanPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($plan.schema -ne 1 -or $plan.transaction -notmatch '^[a-zA-Z0-9-]{1,96}$') {
        throw 'Invalid update plan'
    }
    $exe = [System.IO.Path]::GetFullPath($plan.executable)
    $installation = Split-Path -Parent $exe
    if (-not (Same-Path $installation $plan.installation)) { throw 'Installation path changed' }
    if (Same-Path $transaction $installation) { throw 'Helper cannot run inside installation' }
    if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Installed application is missing' }
    if (-not (Test-Path -LiteralPath (Join-Path $installation 'unins000.exe') -PathType Leaf)) {
        throw 'This directory is not an installer-managed installation'
    }
    if ($plan.version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+([+-][a-zA-Z0-9.-]+)?$' -or
        $plan.original_version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+([+-][a-zA-Z0-9.-]+)?$' -or
        $plan.sha256 -notmatch '^[a-fA-F0-9]{64}$') { throw 'Invalid package identity' }
    if ($plan.participants.Count -lt 1 -or $plan.participants.Count -gt 32) { throw 'Invalid participant count' }
    # 先取得内核生命周期锁再宣告 ready:崩溃自动释放;空文件本身不是锁。
    $guard = [System.IO.FileStream]::new($plan.guard_path, [System.IO.FileMode]::OpenOrCreate,
        [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
    foreach ($participant in $plan.participants) {
        $process = [System.Diagnostics.Process]::GetProcessById([int]$participant.pid)
        $null = $process.Handle # 先取内核对象,再核对身份。
        if (-not (Same-Path $process.MainModule.FileName $exe)) { throw 'Participant executable differs' }
        if ($process.StartTime.ToUniversalTime().ToFileTimeUtc().ToString() -ne $participant.created) {
            throw 'Participant process identity changed'
        }
        $handles += $process
    }
    Check-UnpreparedProcesses
    # 校验到安装全程保持安装包只读句柄,防替换。
    $installerGuard = [System.IO.FileStream]::new($plan.installer, [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
    if ($installerGuard.Length -ne $plan.bytes) { throw 'Installer size changed' }
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { $actual = [BitConverter]::ToString($sha.ComputeHash($installerGuard)).Replace('-', '') }
    finally { $sha.Dispose() }
    if ($actual -ne $plan.sha256) { throw 'Installer checksum changed' }
    $originalDigest = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
    if ((Read-Version $exe) -notmatch ('^slterm ' + [regex]::Escape($plan.original_version) + '(\s|$)')) {
        throw 'Original application version differs'
    }
    Write-State 'ready.json' @{ transaction = $plan.transaction; helper = $PID }

    # 等 commit(唯一安装权威)或 cancel;应用退出而未 commit = 放弃。
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    $commitPath = Join-Path $transaction 'commit.json'
    while (-not (Test-Path -LiteralPath $commitPath)) {
        if ([DateTime]::UtcNow -ge $deadline -or (Test-Path -LiteralPath (Join-Path $transaction 'cancel.json'))) {
            throw 'Installation was not committed'
        }
        if (@($handles | Where-Object { -not $_.HasExited }).Count -eq 0) {
            throw 'Application exited before committing the update'
        }
        Start-Sleep -Milliseconds 100
    }
    $commit = Get-Content -LiteralPath $commitPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($commit.transaction -ne $plan.transaction) { throw 'Commit identity differs' }
    $committed = $true
    # 应用先保存并退出,helper 再覆盖安装;绝不杀用户进程。
    foreach ($process in $handles) {
        if (-not $process.WaitForExit(60000)) { throw 'Application has not exited; installation aborted' }
    }
    Check-UnpreparedProcesses
    Wait-RuntimeHelperFiles
    $setupLog = Join-Path $transaction 'installer.log'
    $arguments = '/SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /NOCLOSEAPPLICATIONS /NORESTARTAPPLICATIONS' +
        ' /DIR="' + $installation + '" /LOG="' + $setupLog + '"'
    $setup = Start-Process -FilePath $plan.installer -ArgumentList $arguments -PassThru
    $setup.WaitForExit()
    if ($setup.ExitCode -ne 0) { throw "Installer failed with exit code $($setup.ExitCode)" }
    $reported = Read-Version $exe
    if ($reported -notmatch ('^slterm ' + [regex]::Escape($plan.version) + '(\s|$)')) {
        throw 'Installed application did not report the expected version'
    }
    $installedDigest = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
    # setup 成功 + 版本回读一致即权威;主 exe 摘要变化不是同版本成功的必要条件
    # (repair 可能替换锁定的 helper)。
    Write-State 'result.json' @{
        transaction = $plan.transaction; success = $true; version = $plan.version
        executable_sha256 = $installedDigest
    }
    $guard.Dispose()
    $guard = $null
    Launch-Workspace
} catch {
    $failure = $_.Exception.Message
    # 只在原参与者全部退出且旧二进制未变时重启原版本;这是恢复,不是
    # 声称安装器已回滚。
    $recoverOriginal = $false
    if ($committed -and $originalDigest -and
        @($handles | Where-Object { -not $_.HasExited }).Count -eq 0) {
        try {
            Check-UnpreparedProcesses
            $recoverOriginal = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash -eq $originalDigest -and
                (Read-Version $exe) -match ('^slterm ' + [regex]::Escape($plan.original_version) + '(\s|$)')
        } catch { $recoverOriginal = $false }
    }
    try { Write-State 'result.json' @{
        transaction = $plan.transaction; success = $false; committed = $committed
        recovered_original = $recoverOriginal; error = $failure
    } }
    catch { [Console]::Error.WriteLine('Could not persist update failure') }
    if ($recoverOriginal) {
        if ($guard) { $guard.Dispose(); $guard = $null }
        try { Launch-Workspace }
        catch { [Console]::Error.WriteLine('Could not restart the original application') }
    }
    exit 1
} finally {
    foreach ($process in $handles) { $process.Dispose() }
    if ($installerGuard) { $installerGuard.Dispose() }
    if ($guard) { $guard.Dispose() }
}
