# Launches the release build, measures startup time and RAM, and saves a screenshot.
# Usage: powershell -ExecutionPolicy Bypass -File scripts\measure.ps1
param([string]$Exe = "$PSScriptRoot\..\target\release\nebula-reader.exe", [string]$Shot = "$env:TEMP\nebula-shot.png")
$exe = (Resolve-Path $Exe).Path
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$sw = [Diagnostics.Stopwatch]::StartNew()
$p = Start-Process $exe -PassThru
while ($p.MainWindowHandle -eq 0 -and $sw.ElapsedMilliseconds -lt 15000) { Start-Sleep -Milliseconds 50; $p.Refresh() }
"Window visible after: $($sw.ElapsedMilliseconds) ms"
Start-Sleep -Seconds 4
$ids = @($p.Id)
# Process tree: app + child WebView2 processes
$all = Get-CimInstance Win32_Process
$queue = @($p.Id); $tree = @($p.Id)
while ($queue.Count) { $cur=$queue[0]; $queue=$queue[1..($queue.Count)]; foreach($c in $all|?{$_.ParentProcessId -eq $cur}){ $tree+=$c.ProcessId; $queue+=$c.ProcessId } }
$procs = Get-Process -Id $tree -ErrorAction SilentlyContinue
$procs | Select-Object Id, ProcessName, @{n='PrivateMB';e={[math]::Round($_.PrivateMemorySize64/1MB,1)}}, @{n='WorkingSetMB';e={[math]::Round($_.WorkingSet64/1MB,1)}} | Format-Table -AutoSize | Out-String
"TOTAL private MB (app + WebView2): " + [math]::Round(($procs | Measure-Object PrivateMemorySize64 -Sum).Sum/1MB,1)
"TOTAL working set MB: " + [math]::Round(($procs | Measure-Object WorkingSet64 -Sum).Sum/1MB,1)
$sig = '[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); public struct RECT { public int L,T,R,B; }'
$w = Add-Type -MemberDefinition $sig -Name W -Namespace U -PassThru
[U.W]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 500
$r = New-Object U.W+RECT; [U.W]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
$bmp = New-Object Drawing.Bitmap ($r.R-$r.L), ($r.B-$r.T)
$g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
$bmp.Save($Shot)
"Window size: $($r.R-$r.L)x$($r.B-$r.T)"
Stop-Process -Id $p.Id
