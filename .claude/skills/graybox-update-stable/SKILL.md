---
name: graybox-update-stable
description: Use when Adriel asks to update his installed Graybox ("atualiza o graybox", "versão estável", "instala a versão nova") from the dev checkout.
---

# Update the installed Graybox

Installed copy: `C:\Users\Adriel\Apps\Graybox\` (`graybox.exe`, `graybox-anterior.exe`, `versao.txt`,
and his own `Atualizar Graybox.ps1`, which does the same by hand). Do the steps in PowerShell.

1. **He must have it closed.** `Get-Process graybox -ErrorAction SilentlyContinue`. If running, stop
   here and ask him in Portuguese to close it. **Never kill it.**
2. Commit/stash nothing for him. Check `git status` is what he expects; build what is on disk.
3. Build (minutes; run in background):
   `cargo build --release -p figcraft`. If it fails, stop: the installed version stays untouched.
4. Install:
```powershell
$here="C:\Users\Adriel\Apps\Graybox"; $repo="C:\Users\Adriel\Documents\graybox"
if (Test-Path "$here\graybox.exe") { Copy-Item "$here\graybox.exe" "$here\graybox-anterior.exe" -Force }
Copy-Item "$repo\target\release\figcraft.exe" "$here\graybox.exe" -Force
$commit = git -C $repo log -1 --format="%h %s (%cd)" --date=short
Set-Content "$here\versao.txt" $commit -Encoding utf8
```
5. **Smoke test on his file without changing it** (his file = the one he names, else the newest
   `.figcraft` under `C:\Users\Adriel\Documents\graybox projects\`):
```powershell
$f = "<his file>"; $before = (Get-Item $f).LastWriteTimeUtc
# free port + Start-Process "$here\graybox.exe" "--control $port `"$f`"" -PassThru  (client: see graybox-app-check)
# Ctl 'document.inspect' @{depth=1}  -> expect dirty=false and history.undo empty
Stop-Process -Id $app.Id          # your own process only
(Get-Item $f).LastWriteTimeUtc -eq $before   # must be True
```
   Send **no** edits, clicks or saves. If `dirty` is true, undo is not empty, or the timestamp moved,
   report it as a bug (opening a file must not touch it) and tell him `graybox-anterior.exe` is the way back.
6. Tell him (Portuguese): the new `versao.txt` line, that he can reopen Graybox, and how to go back
   (rename `graybox-anterior.exe`).

Do not commit anything for this task; it changes nothing in the repo.
