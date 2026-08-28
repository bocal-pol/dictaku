#Requires -Version 5.1
<#
.SYNOPSIS
    Télécharge un modèle Whisper GGML depuis HuggingFace pour Dictaku.

.DESCRIPTION
    Télécharge le modèle Whisper GGML sélectionné, vérifie son intégrité
    via SHA256, et l'installe dans %APPDATA%\dictaku\models\.

.PARAMETER Model
    Modèle à télécharger : tiny | base | small (défaut : base)

.PARAMETER ModelsDir
    Répertoire de destination (défaut : %APPDATA%\dictaku\models)

.EXAMPLE
    .\download-model.ps1 -Model base
    .\download-model.ps1 -Model small -ModelsDir "D:\models\whisper"

.NOTES
    Auteur  : Pascal Dengis
    Licence : MIT 2026
    Version : 1.0.0
#>
[CmdletBinding()]
param(
    [ValidateSet('tiny', 'base', 'small')]
    [string]$Model = 'base',

    [string]$ModelsDir = "$env:APPDATA\dictaku\models"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ── Configuration des modèles ────────────────────────────────────────────────

$ModelConfig = @{
    tiny  = @{
        Filename = 'ggml-tiny.bin'
        URL      = 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin'
        SHA256   = 'be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21'
        SizeMB   = 39
    }
    base  = @{
        Filename = 'ggml-base.bin'
        URL      = 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin'
        SHA256   = '60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe'
        SizeMB   = 74
    }
    small = @{
        Filename = 'ggml-small.bin'
        URL      = 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin'
        SHA256   = '1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b'
        SizeMB   = 244
    }
}

# ── Fonctions ────────────────────────────────────────────────────────────────

function Write-Header {
    Write-Host ""
    Write-Host "  Dictaku — Téléchargement modèle Whisper" -ForegroundColor Green
    Write-Host "  ─────────────────────────────────────────" -ForegroundColor DarkGreen
}

function Get-FileSHA256 {
    param([string]$Path)
    $hash = Get-FileHash -Path $Path -Algorithm SHA256
    return $hash.Hash.ToLower()
}

function Show-Progress {
    param(
        [long]$BytesReceived,
        [long]$TotalBytes,
        [string]$Activity
    )
    if ($TotalBytes -gt 0) {
        $pct = [int](($BytesReceived / $TotalBytes) * 100)
        $mb  = [math]::Round($BytesReceived / 1MB, 1)
        $total = [math]::Round($TotalBytes / 1MB, 1)
        Write-Progress -Activity $Activity `
            -Status "$mb MB / $total MB ($pct%)" `
            -PercentComplete $pct
    }
}

# ── Script principal ─────────────────────────────────────────────────────────

Write-Header

$cfg      = $ModelConfig[$Model]
$destPath = Join-Path $ModelsDir $cfg.Filename

Write-Host ""
Write-Host "  Modèle   : $Model (~$($cfg.SizeMB) MB)" -ForegroundColor Cyan
Write-Host "  Source   : $($cfg.URL)" -ForegroundColor DarkGray
Write-Host "  Dest     : $destPath" -ForegroundColor DarkGray
Write-Host ""

# Vérification si le modèle existe déjà.
if (Test-Path $destPath) {
    Write-Host "  Vérification de l'intégrité du fichier existant…" -ForegroundColor Yellow
    $existing = Get-FileSHA256 -Path $destPath
    if ($existing -eq $cfg.SHA256) {
        Write-Host "  ✓ Modèle déjà présent et valide — aucun téléchargement requis." -ForegroundColor Green
        Write-Host ""
        exit 0
    } else {
        Write-Host "  ⚠ SHA256 invalide — re-téléchargement du fichier." -ForegroundColor Yellow
    }
}

# Création du répertoire de destination.
if (-not (Test-Path $ModelsDir)) {
    Write-Host "  Création du dossier : $ModelsDir" -ForegroundColor DarkGray
    New-Item -ItemType Directory -Path $ModelsDir -Force | Out-Null
}

# Téléchargement avec barre de progression.
Write-Host "  Téléchargement en cours…" -ForegroundColor Cyan

$tempPath = "$destPath.tmp"

try {
    $webClient = New-Object System.Net.WebClient

    # Callback de progression.
    $webClient.add_DownloadProgressChanged({
        param($sender, $e)
        Show-Progress -BytesReceived $e.BytesReceived `
            -TotalBytes $e.TotalBytesToReceive `
            -Activity "Téléchargement ggml-$Model.bin"
    })

    # Téléchargement asynchrone converti en synchrone.
    $task = $webClient.DownloadFileTaskAsync($cfg.URL, $tempPath)
    $task.Wait()

} catch {
    Write-Progress -Activity "Téléchargement" -Completed
    if (Test-Path $tempPath) { Remove-Item $tempPath -Force }
    Write-Host ""
    Write-Host "  ✗ Erreur de téléchargement : $_" -ForegroundColor Red
    Write-Host "    Vérifiez votre connexion Internet et réessayez." -ForegroundColor DarkRed
    exit 1
} finally {
    Write-Progress -Activity "Téléchargement" -Completed
    if ($null -ne $webClient) { $webClient.Dispose() }
}

# Vérification SHA256.
Write-Host "  Vérification SHA256…" -ForegroundColor Cyan
$computed = Get-FileSHA256 -Path $tempPath
$expected = $cfg.SHA256

Write-Host "  Calculé  : $computed" -ForegroundColor DarkGray
Write-Host "  Attendu  : $expected" -ForegroundColor DarkGray

if ($computed -ne $expected) {
    Remove-Item $tempPath -Force
    Write-Host ""
    Write-Host "  ✗ Vérification SHA256 échouée — fichier corrompu ou différent du modèle attendu." -ForegroundColor Red
    Write-Host "    Réessayez le téléchargement. Si l'erreur persiste, vérifiez les checksums officiels." -ForegroundColor DarkRed
    exit 1
}

# Déplacement du fichier temporaire vers la destination finale.
Move-Item -Path $tempPath -Destination $destPath -Force

Write-Host ""
Write-Host "  ✓ Modèle installé avec succès !" -ForegroundColor Green
Write-Host "    Chemin : $destPath" -ForegroundColor DarkGreen
Write-Host "    Taille : $([math]::Round((Get-Item $destPath).Length / 1MB, 1)) MB" -ForegroundColor DarkGreen
Write-Host ""
