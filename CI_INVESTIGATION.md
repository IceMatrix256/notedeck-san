CI investigation notes

Date: 2026-02-21T00:50:00Z

Summary
-------
Fixed compile errors in crates/notedeck_ui/src/note/mod.rs (egui input() API usage) and pushed changes to feature/reactions. Also added workflow_dispatch to .github/workflows/android-debug.yml on master to enable manual dispatch.

Observed behavior
-----------------
- CI runs are created for pushes to feature/reactions but complete immediately with conclusion: failure and show zero jobs.
- Example run IDs: 22246819065, 22246884341, 22246914068.
- Attempts to fetch logs programmatically (GitHub API / gh CLI) returned 404 or "log not found".

Actions taken
-------------
- Fixed egui API usage (use closure with ctx.input(|i| ...)) and committed to feature/reactions.
- Pushed an empty commit to retrigger the workflow.
- Added workflow_dispatch to .github/workflows/android-debug.yml on master and pushed to master to allow manual dispatch.
- Attempted manual dispatch via gh API; GitHub still returned 422 (workflow does not have 'workflow_dispatch') in some attempts — likely propagation delay.
- Polled runs and attempted to download logs; gh API returned Not Found for logs.

Next steps / Recommendations
---------------------------
1. Check the GitHub Actions page for one of the run IDs above and look for detailed failure reasons (YAML parse errors or skipped jobs). The web UI may show a parsing error that the API does not expose.
2. Verify repository Actions settings: ensure Actions are enabled for this fork and that workflows can run for pushes and manual dispatch.
3. If needed, grant a GitHub token with repo scope or add me with access so I can fetch logs and rerun workflows.
4. Alternatively, open a PR against the upstream repository (if available) so CI runs in the upstream context where logs were previously available.

I will continue polling for logs and retry dispatch for a short period, but please review the Actions UI when available.
