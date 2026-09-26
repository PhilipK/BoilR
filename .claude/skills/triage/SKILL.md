---
name: triage
description: Triage BoilR issues: label, reproduce by reading code, reply, deduplicate, or close stale ones. Use for new issues and for the stale-issue sweep.
---

## Per issue

1. Read the issue and all comments. Classify: `bug`, `enhancement`, `question`, `documentation`, and `duplicate` where it applies. Add the platform in the title if missing is not your job; do not edit user text.
2. For bugs, find the code path (platform module, `src/steam/`, `src/ui/`) and state in a reply what you believe is happening, or what information would tell you (OS, install type Flatpak/native/AppImage, launcher and its install type, the `--no-ui` output). Label `need more info` when you asked for something.
3. Link duplicates to the older issue and close the newer one, unless the newer one has the better report, in which case close the older.
4. If an open PR fixes the issue, link it.
5. A bug with a clear cause and small fix is a roadmap candidate: add it to the log issue roadmap.

## Stale sweep

An issue is **stale** when it has had no activity for 12 months. Close stale issues with a comment saying BoilR has changed a lot since, inviting the reporter to reopen with details on the latest release if it still happens. Before closing, check whether it is still plausibly relevant: keep open (and label `help wanted` if fitting) feature requests that match the roadmap or have clear user demand (several thumbs-up or commenters), and bugs whose cause you can still see in current code.

Issues labelled `need more info` with no reply for 60 days: close with the same reopen invitation.

## Tone

Short, friendly, concrete. Users of a free tool owe nothing; thank them for reports.
