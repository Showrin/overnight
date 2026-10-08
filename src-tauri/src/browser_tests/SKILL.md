---
name: browser-test
description: Test this project's web frontend in a real Chrome browser on the user's machine (Claude in Chrome, run by Overnight). Use after building or changing anything user-facing in a web app, to verify it actually works in a browser before calling the work done.
---

# Browser testing through Overnight

You can't run a browser in this sandbox. Overnight, the app on the user's machine, can: you hand it a test plan, a separate agent on the host runs the plan in Chrome, and you get a report back.

## 1. Make the app reachable

Run `overnight-browser-test check` (it's in `~/.local/bin`). It prints the `target`:

- `sandbox`: the host browser opens the app running **in this sandbox**. Start the dev server listening on all interfaces (`0.0.0.0`, not `127.0.0.1`) on port `$OVERNIGHT_BROWSER_PORT` (pass `--port` if you use another one), and leave it running.
- `external`: the app runs on the user's own machine from your branch. **Commit your work to the branch first.** If `check` shows `"host_prep": true`, Overnight checks the branch out on the host and starts the servers itself. Otherwise tell the user which branch to check out; the test waits until they start the servers and press Start in Overnight.

## 2. Write the test plan

Write a Markdown file for another agent that has never seen this code. It can only use the browser, so include:

- **What changed and why**, in a few sentences.
- **Where to go**: paths relative to the app root (e.g. `/settings/profile`). Don't write full URLs; the host agent is told the base URL.
- **Preconditions**: test accounts and passwords, seed data, feature flags, and what state the app should be in.
- **Checks**: numbered steps, each with the action and the exact expected result.
- **Edge cases** worth trying: empty input, errors, long text, narrow window.
- **What else to watch**: console errors, failed network requests, layout problems.

## 3. Run it

```bash
overnight-browser-test run browser-test-plan.md
```

This blocks until the report is ready (polling every 10s, default timeout 60 minutes; use `--timeout`) and prints the report as Markdown. If it times out, resume with `overnight-browser-test wait <id>`. Exit code 2 means the test itself failed to run; read the error, fix the cause (e.g. the dev server wasn't up), and try again.

## 4. Act on the report

Fix every issue the report lists and consider its suggestions. Then run the plan again until it passes, or explain to the user which findings you deliberately left alone and why.
