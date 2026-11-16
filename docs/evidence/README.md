# Tracked Verification Evidence

Cyron's documentation must stay verifiable from a clean clone. CI runs
`npm run docs:check`, which fails on links to files that are not committed.

## Rules

1. **Receipts ship; selected native captures may ship.** Verification receipts live
   in this directory as Markdown. They record the date, the commit or tree state, the
   exact command, the observed result, and every limitation. A small, sanitized
   screenshot may be committed beside its receipt when it is the direct evidence for a
   native desktop check; bulk captures, recordings and anything containing unrelated
   personal data stay local.
2. **Never link to local-only artifacts.** `.unlazy/`, `.chryypt/`, `tmp/`,
   `screenshots/`, build output, and `node_modules/` are gitignored. A Markdown
   link into those directories is a documentation defect, because readers and CI
   cannot open it. Cite a digest or describe the artifact instead.
3. **Do not upgrade a limitation into a pass.** A receipt keeps "not exercised",
   "headless CI: unverified", and "fixture only" exactly as observed.
4. **A receipt is not a re-run.** Transcribing an earlier local result is
   allowed when it is labeled with its original date; the audit that transcribes
   it adds its own fresh checks separately.

`npm run docs:check` enforces rule 2 and validates every link in this directory.
