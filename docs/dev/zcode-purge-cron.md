# `purge_zcode_refs.sh` — schedule and run

The Z.ai ZCode integration (which inspired the design of
`claude-checkpoint` and `claude-specialized-agents`) periodically
writes per-session root commits into `refs/zcode/checkpoints/...`.
These refs:

- Pollute `git log --all` with hundreds of `ZCode Checkpoint` commits.
- Carry Chrome cache + audit.toml as their tree, bloating
  `.git/objects/`.
- Cannot be filtered out by a `git config receive.denyDeleteCurrent`
  because the writes happen locally (no push).

We have three layers of defense, in order of strength:

1. `scripts/git-hooks/pre-receive` (server-side) — rejects any push
   of a `refs/zcode/**` ref.  See `scripts/install-hooks.sh`.
2. `scripts/purge_zcode_refs.sh` / `.ps1` — deletes the refs and
   `git gc --prune=now`.  Run manually or on a schedule.
3. The `.gitignore` and `plans/archive/zcode-checkpoints-archive.md`
   documentation — passive.

## Recommended schedule

On a developer workstation (Windows / macOS / Linux) running the
project, schedule the purge script once a day.

### Windows (Task Scheduler)

```powershell
# Run as the current user, daily at 03:30 local.
$action = New-ScheduledTaskAction `
  -Execute "pwsh.exe" `
  -Argument "-File C:\path\to\repo\scripts\purge_zcode_refs.ps1" `
  -WorkingDirectory "C:\path\to\repo"
$trigger = New-ScheduledTaskTrigger -Daily -At "03:30"
Register-ScheduledTask `
  -TaskName "purge-zcode-refs-daily" `
  -Action $action `
  -Trigger $trigger `
  -Description "Strip ZCode checkpoint refs that the Z.ai IDE integration keeps writing into the local refdb."
```

### Linux (cron)

```sh
# /etc/cron.d/purge-zcode-refs — adjust user and path
30 3 * * *  yanzhi  cd /home/yanzhi/remote-code-rust && ./scripts/purge_zcode_refs.sh >> ~/.cache/purge-zcode.log 2>&1
```

### macOS (launchd)

`~/Library/LaunchAgents/com.remote-code.purge-zcode.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
 "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key><string>com.remote-code.purge-zcode</string>
    <key>ProgramArguments</key>
    <array>
      <string>/bin/bash</string>
      <string>/path/to/repo/scripts/purge_zcode_refs.sh</string>
    </array>
    <key>StartCalendarInterval</key>
    <dict><key>Hour</key><integer>3</integer><key>Minute</key><integer>30</integer></dict>
    <key>StandardOutPath</key><string>/tmp/purge-zcode.log</string>
    <key>StandardErrorPath</key><string>/tmp/purge-zcode.log</string>
  </dict>
</plist>
```

## Verification

After install, you should see a daily entry like:

```
Found 2 refs/zcode/** ref(s):
  refs/zcode/checkpoints/9ed25038d869/<uuid>
  refs/zcode/checkpoints/9ed25038d869/<uuid>

Purge complete: 0 refs/zcode/** remaining.
```

If the count keeps growing, the Z.ai IDE is writing faster than the
schedule can keep up.  In that case, increase the cron frequency
(every 15 minutes) or, better, **disable the Z.ai git integration
in the IDE** — the integration is unnecessary; ZCode checkpoints
have no functional value for this codebase.
