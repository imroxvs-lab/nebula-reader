# Descarga el motor PDFium (pdfium.dll) usado para leer PDF y lo deja en
# src-tauri/resources/pdfium/, desde donde se incluye en el instalador.
#
# Binarios: https://github.com/bblanchon/pdfium-binaries (licencia BSD-3/Apache-2.0).
# La versión y el hash están fijados: para actualizar, cambia ambos valores.
#
# Uso: powershell -ExecutionPolicy Bypass -File scripts\fetch-pdfium.ps1
$ErrorActionPreference = 'Stop'

$Version = '8179'
$Sha256 = '836A77C9053F624839C66010CDDEA7B675628A36B8831D2330178BB169D241B6'
$Url = "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/$Version/pdfium-win-x64.tgz"
$Destination = Join-Path $PSScriptRoot '..\src-tauri\resources\pdfium'

if ((Test-Path (Join-Path $Destination 'pdfium.dll')) -and
    ((Get-Content (Join-Path $Destination 'VERSION') -ErrorAction SilentlyContinue) -match "BUILD=$Version")) {
    "PDFium $Version ya está instalado."
    exit 0
}

$archive = Join-Path $env:TEMP "pdfium-$Version-win-x64.tgz"
"Descargando PDFium $Version..."
Invoke-WebRequest -Uri $Url -OutFile $archive -UseBasicParsing
$sha = [System.Security.Cryptography.SHA256]::Create()
$stream = [System.IO.File]::OpenRead($archive)
try { $hash = -join ($sha.ComputeHash($stream) | ForEach-Object { $_.ToString('X2') }) } finally { $stream.Dispose(); $sha.Dispose() }
if ($hash -ne $Sha256) {
    Remove-Item $archive
    throw "El hash de PDFium no coincide (esperado $Sha256, obtenido $hash)."
}

$extract = Join-Path $env:TEMP "pdfium-$Version-extract"
Remove-Item $extract -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $extract | Out-Null
tar -xzf $archive -C $extract
New-Item -ItemType Directory -Path $Destination -Force | Out-Null
Copy-Item (Join-Path $extract 'bin\pdfium.dll') $Destination -Force
Copy-Item (Join-Path $extract 'LICENSE') (Join-Path $Destination 'LICENSE') -Force
Copy-Item (Join-Path $extract 'VERSION') (Join-Path $Destination 'VERSION') -Force
Remove-Item $extract, $archive -Recurse -Force
"PDFium $Version instalado en $((Resolve-Path $Destination).Path)"
