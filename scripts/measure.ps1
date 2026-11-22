# Lanza la app, mide el arranque y la memoria (app + procesos WebView2) y guarda una captura.
#
# Uso:
#   powershell -ExecutionPolicy Bypass -File scripts\measure.ps1 [-Wait 4] [-Exe <ruta>] [-Shot <png>]
#
# Métricas:
#   - privateWS: working set privado = columna "Memoria" del Administrador de tareas (métrica principal).
#   - commit:    memoria privada comprometida. En el proceso GPU incluye reservas de Direct3D que no
#                ocupan RAM física, por eso es mucho mayor que privateWS.
param(
    [int]$Wait = 4,
    [string]$Exe = "$PSScriptRoot\..\target\release\nebula-reader.exe",
    [string]$Shot = "$env:TEMP\nebula-shot-$(Get-Date -Format 'yyyyMMdd-HHmmss').png"
)

$ErrorActionPreference = 'Stop'
$exePath = (Resolve-Path $Exe).Path
Add-Type -AssemblyName System.Drawing
Add-Type -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
public struct RECT { public int L, T, R, B; }
'@ -Name W -Namespace NebulaMeasure

$sw = [Diagnostics.Stopwatch]::StartNew()
$app = Start-Process $exePath -PassThru
while ($app.MainWindowHandle -eq 0 -and $sw.ElapsedMilliseconds -lt 15000) {
    Start-Sleep -Milliseconds 20
    $app.Refresh()
}
"Ventana visible en: $($sw.ElapsedMilliseconds) ms"
Start-Sleep -Seconds $Wait

# Árbol de procesos: la app y todos los procesos WebView2 que cuelgan de ella.
$all = Get-CimInstance Win32_Process
$tree = [System.Collections.Generic.List[int]]::new()
$queue = [System.Collections.Generic.Queue[int]]::new()
$queue.Enqueue($app.Id)
while ($queue.Count -gt 0) {
    $current = $queue.Dequeue()
    $tree.Add($current)
    $all | Where-Object { $_.ParentProcessId -eq $current } | ForEach-Object { $queue.Enqueue($_.ProcessId) }
}

$perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process
$totalWs = 0; $totalCommit = 0
$rows = foreach ($proc in $all | Where-Object { $tree.Contains($_.ProcessId) }) {
    $type = if ($proc.CommandLine -match '--type=([\w-]+)') { $Matches[1] } else { 'principal' }
    if ($proc.CommandLine -match '--utility-sub-type=[\w]+\.mojom\.(\w+)') { $type += ":" + $Matches[1] }
    $gp = Get-Process -Id $proc.ProcessId -ErrorAction SilentlyContinue
    if (-not $gp) { continue }
    $ws = ($perf | Where-Object { $_.IDProcess -eq $proc.ProcessId }).WorkingSetPrivate
    $totalWs += $ws; $totalCommit += $gp.PrivateMemorySize64
    [pscustomobject]@{
        Proceso     = "$($proc.Name) [$type]"
        PrivateWSMB = [math]::Round($ws / 1MB, 1)
        CommitMB    = [math]::Round($gp.PrivateMemorySize64 / 1MB, 1)
    }
}
$rows | Format-Table -AutoSize | Out-String
"TOTAL privateWS (Administrador de tareas): {0:N1} MB" -f ($totalWs / 1MB)
"TOTAL commit: {0:N1} MB" -f ($totalCommit / 1MB)

[NebulaMeasure.W]::SetForegroundWindow($app.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 500
$rect = New-Object NebulaMeasure.W+RECT
[NebulaMeasure.W]::GetWindowRect($app.MainWindowHandle, [ref]$rect) | Out-Null
$bmp = New-Object Drawing.Bitmap ($rect.R - $rect.L), ($rect.B - $rect.T)
[Drawing.Graphics]::FromImage($bmp).CopyFromScreen($rect.L, $rect.T, 0, 0, $bmp.Size)
$bmp.Save($Shot)
"Ventana: $($rect.R - $rect.L)x$($rect.B - $rect.T)"
"Captura: $Shot"

# Cierre ordenado para que la app guarde su estado (p. ej. tamaño de ventana).
$app.CloseMainWindow() | Out-Null
if (-not $app.WaitForExit(5000)) { Stop-Process -Id $app.Id }
