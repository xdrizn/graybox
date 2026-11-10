---
name: graybox-app-check
description: Use when a Graybox UI or canvas change must be verified in the real app (screenshot, click, drag, panel behaviour), before saying a UI task is done.
---

# Check UI in the real app

CLAUDE.md: "for UI work, look at the result". Never test on Adriel's real files: **copy first**.

## 1. Build and scratch copy (PowerShell)
```powershell
cargo build -p figcraft
$scratch = "$env:TEMP\graybox-check"; New-Item -ItemType Directory -Force $scratch | Out-Null
Copy-Item "C:\Users\Adriel\Documents\graybox projects\<project>\<file>.figcraft" "$scratch\test.figcraft"   # or start with no file
```
Never open anything under `Documents\graybox projects\` directly. Never use `graybox.exe` (his installed copy; only `graybox-update-stable` starts it).

## 2. Start the test app in the background
```powershell
$l=[Net.Sockets.TcpListener]::new('127.0.0.1',0); $l.Start(); $port=$l.LocalEndpoint.Port; $l.Stop()
$app = Start-Process "C:\Users\Adriel\Documents\graybox\target\debug\figcraft.exe" -ArgumentList "--control $port `"$scratch\test.figcraft`"" -PassThru
Start-Sleep 5   # then poll doc.list until it answers
```

## 3. Minimal client (one JSON line in, one reply line out)
```powershell
function Ctl($method, $params) {
  $c = [Net.Sockets.TcpClient]::new('127.0.0.1', $port); $s = $c.GetStream()
  $w = [IO.StreamWriter]::new($s); $r = [IO.StreamReader]::new($s)
  $w.WriteLine((@{id=1; method=$method; params=$params} | ConvertTo-Json -Depth 12 -Compress)); $w.Flush()
  $r.ReadLine(); $c.Close()
}
Ctl 'engine.execute' @{command='shape.rectangle'; params=@{x=10;y=10;width=100;height=50}}
Ctl 'ui.screenshot' @{path="$scratch\shot.png"}
```
Then `Read` the PNG and look at it. Details: `docs/control-protocol.md`.

## Methods you need
| Method | Use |
|---|---|
| `engine.execute {command, params}` | any command (same ids as menus) |
| `ui.pointer {kind: down/move/up/click/double/drag, x, y, to?, shift?, alt?}` | canvas tools; **page coordinates** |
| `ui.input {events: [...]}` | raw window input for panels/popovers; **window points**: `{move:[x,y]}`, `{down:[x,y]}`, `{up:[x,y]}`, `{key:"Escape"}`, `{text:"…"}`, `{hold:{alt:true}}` |
| `ui.screenshot {path}` | whole window PNG |
| `document.inspect {depth?, outline?}` / `node.get {id}` | assert state (`dirty`, `selection`, `history.undo`) |
| `ui.inspect {}` | panels, tool, canvas rect (find where to click) |

Rules: send a press and its release as **separate** `ui.input` calls; `hold` modifiers in an earlier call
than the click that needs them; one `Ctl` per step, then screenshot. Prefer asserting with
`document.inspect` over eyeballing.

## 4. Stop only your test app
```powershell
if ($app -and -not $app.HasExited) { Stop-Process -Id $app.Id }
Remove-Item $scratch -Recurse -Force
```
Only stop a process whose `Path` is under `C:\Users\Adriel\Documents\graybox\target`. Never touch
`graybox.exe`. Delete the `target\debug` dir only if disk is tight and Adriel agrees.
