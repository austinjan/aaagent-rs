# Start server with environment variables from .env
$ErrorActionPreference = "Stop"

Write-Host "Starting aaagent server with .env configuration" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

# Load API keys from .env file
$envFile = ".env"
if (Test-Path $envFile) {
    Write-Host "Loading environment from .env..." -ForegroundColor Yellow
    Get-Content $envFile | ForEach-Object {
        if ($_ -match '^\s*([^#][^=]*?)\s*=\s*(.+?)\s*$') {
            $key = $matches[1]
            $value = $matches[2]
            Set-Item -Path "env:$key" -Value $value

            # Mask API keys in output
            if ($key -like "*API_KEY*" -or $key -like "*SECRET*") {
                $maskedValue = $value.Substring(0, [Math]::Min(8, $value.Length)) + "..."
                Write-Host "  ✓ $key = $maskedValue" -ForegroundColor Green
            } else {
                Write-Host "  ✓ $key = $value" -ForegroundColor Green
            }
        }
    }
    Write-Host ""
} else {
    Write-Host "Warning: .env file not found!" -ForegroundColor Red
    Write-Host "Please create a .env file with your API keys:" -ForegroundColor Yellow
    Write-Host "  OPENAI_API_KEY=sk-..." -ForegroundColor Gray
    Write-Host "  ANTHROPIC_API_KEY=sk-ant-..." -ForegroundColor Gray
    Write-Host "  GOOGLE_API_KEY=..." -ForegroundColor Gray
    Write-Host ""
    exit 1
}

# Check which provider is available
$hasProvider = $false
if ($env:OPENAI_API_KEY) {
    Write-Host "✓ OpenAI provider available" -ForegroundColor Green
    $hasProvider = $true
}
if ($env:ANTHROPIC_API_KEY) {
    Write-Host "✓ Anthropic provider available" -ForegroundColor Green
    $hasProvider = $true
}
if ($env:GOOGLE_API_KEY) {
    Write-Host "✓ Google/Gemini provider available" -ForegroundColor Green
    $hasProvider = $true
}

if (-not $hasProvider) {
    Write-Host ""
    Write-Host "ERROR: No LLM provider API key found!" -ForegroundColor Red
    Write-Host "Please add at least one API key to .env file" -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "Starting server..." -ForegroundColor Yellow
Write-Host "Press Ctrl+C to stop" -ForegroundColor Gray
Write-Host ""
Write-Host "Server logs:" -ForegroundColor Cyan
Write-Host "============" -ForegroundColor Cyan

# Start the server
cargo run --bin aaagent-serve
