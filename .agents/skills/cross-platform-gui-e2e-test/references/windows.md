# Windows AgentBay Run

Use a fresh AgentBay Windows Computer Use virtual desktop. Record its session and image identifiers before transferring the artifact.

## Transfer and launch

Upload the verified archive to `C:\ropy-e2e\ropy.zip` with the AgentBay filesystem API. Use shell or command APIs for setup; do not type long PowerShell commands through remote-desktop automation because keyboard layouts can corrupt punctuation.

Run an equivalent non-interactive PowerShell sequence:

```powershell
$ErrorActionPreference = 'Stop'
$root = 'C:\ropy-e2e'
$zip = Join-Path $root 'ropy.zip'
$expected = '<local-sha256-lowercase>'
$actual = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw "SHA-256 mismatch: $actual" }
$app = Join-Path $root 'app'
Expand-Archive -Path $zip -DestinationPath $app -Force
$exe = Join-Path $app 'ropy.exe'
if (-not (Test-Path $exe)) { throw "Missing $exe" }
$process = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 3
if ($process.HasExited) { throw "ropy.exe exited with $($process.ExitCode)" }
$process.Id
```

Use Notepad for the shared marker scenario. Treat an alive process with no initial window as expected, then send `Control+Shift+D`.

## Troubleshoot and clean up

- If PowerShell 5 cannot download from GitHub, keep using controller-side upload; do not weaken TLS settings.
- If typed commands replace `:` or other punctuation, use the command API.
- If the archive arrives but the session expires before extraction, use direct file upload and an idle timeout of at least `10` minutes.
- Collect JSONL logs from `%APPDATA%\ropy\logs` on failure.
- Stop the test process by its recorded PID, then release the AgentBay session and verify that it is gone.

Current official references: [Windows Computer Use](https://help.aliyun.com/en/agentbay/developer-reference/computer-use-windows-server-2022), [desktop UI automation](https://help.aliyun.com/en/agentbay/developer-reference/ui-automation), and [window management](https://help.aliyun.com/en/agentbay/support/window-management).
