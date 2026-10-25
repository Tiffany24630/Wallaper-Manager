param(
    [Parameter(Mandatory = $true)]
    [string]$Executable,
    [int]$TimeoutSeconds = 30
)

$ErrorActionPreference = "Stop"

if ($TimeoutSeconds -lt 1) {
    throw "TimeoutSeconds debe ser positivo."
}

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$smokeRoot = Join-Path ([System.IO.Path]::GetTempPath()) "lumina-smoke-tests"
$token = "lifecycle-$([guid]::NewGuid().ToString('N'))"
$smokeDirectory = Join-Path $smokeRoot $token

if (Test-Path -LiteralPath $smokeDirectory) {
    throw "El directorio smoke generado ya existe; se detiene sin reutilizarlo."
}

$arguments = @(
    "--lumina-smoke-test=$token",
    "--lumina-smoke-exit-after-init"
)
$process = Start-Process -FilePath $resolvedExecutable -ArgumentList $arguments -PassThru
$processId = $process.Id
$diagnosticsPath = Join-Path $smokeDirectory "lumina-smoke-diagnostics.jsonl"
$markerPath = Join-Path $smokeDirectory ".lumina-smoke-owned"
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$exitRequested = $false
$mounted = $false
$observedEvents = New-Object System.Collections.Generic.List[object]

function Read-SmokeEvents {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        return @()
    }
    foreach ($line in (Get-Content -LiteralPath $Path -ErrorAction Stop)) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }
        try {
            $event = $line | ConvertFrom-Json
        } catch {
            continue
        }
        if ($event.token -ne $token -or [int]$event.pid -ne $processId) {
            throw "Se observó un evento que no pertenece al token o PID de esta ejecución."
        }
        $event
    }
}

while ((Get-Date) -lt $deadline) {
    if (Test-Path -LiteralPath $markerPath) {
        $marker = Get-Content -LiteralPath $markerPath -Raw
        if ($marker -ne "lumina-smoke-root-v1`n") {
            throw "El marcador smoke no coincide con el contrato esperado."
        }
    }
    $events = @(Read-SmokeEvents -Path $diagnosticsPath)
    $observedEvents.Clear()
    foreach ($event in $events) {
        [void]$observedEvents.Add($event)
    }
    $mounted = $null -ne ($events | Where-Object { $_.stage -eq "frontend_app_mounted" })
    $exitRequested = $null -ne ($events | Where-Object { $_.stage -eq "smoke_exit_requested" })
    if ($mounted -and $exitRequested) {
        break
    }
    if ($process.HasExited) {
        throw "La instancia smoke terminó antes de completar la inicialización o solicitar la salida normal. PID=$processId"
    }
    Start-Sleep -Milliseconds 100
}

if (-not $mounted) {
    throw "No se recibió frontend_app_mounted dentro del plazo. PID=$processId"
}
if (-not $exitRequested) {
    throw "No se recibió smoke_exit_requested dentro del plazo. PID=$processId"
}

$terminationDeadline = (Get-Date).AddSeconds($TimeoutSeconds)
while (-not $process.HasExited -and (Get-Date) -lt $terminationDeadline) {
    Start-Sleep -Milliseconds 100
}

if (-not $process.HasExited) {
    throw "La solicitud de salida fue registrada, pero el proceso no terminó dentro del plazo. PID=$processId"
}

[pscustomobject]@{
    token = $token
    pid = $processId
    smoke_directory = $smokeDirectory
    frontend_app_mounted = $mounted
    smoke_exit_requested = $exitRequested
    process_terminated = $process.HasExited
    exit_code = $process.ExitCode
}
