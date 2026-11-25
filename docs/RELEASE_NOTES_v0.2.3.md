# Cyron Uninstaller 0.2.3

Cyron 0.2.3 replaces the Everything IPC client with Cyron's own read-only file index, makes indexing and search noticeably faster, and adds multi-select plus CSV/TXT export to Search. Existing 0.2.2 installations can update from **Settings → About → Software updates**.

## Native file index

- Search now builds and keeps its own index over every fixed local drive. Everything does not need to be installed, running or updated.
- The index stores names, paths, sizes, created/accessed/modified times and attributes. It never reads file contents, follows reparse points or executes a result.
- The cache lives at `%APPDATA%\dev.cyron.uninstaller\search-index.bin`, loads in the background at startup and is rebuilt on demand.
- File-system watchers apply additions, removals, renames and metadata changes while Cyron runs.
- The Everything-style query language remains: AND/OR/NOT, phrases, wildcards, regex, `ext:`, `file:`, `folder:`, `size:`, `dm:`/`dc:`/`da:`, `type:`, `path:`, `attrib:`, `dupe:` and `empty:`.

## Faster indexing and results

- Directory enumeration now runs on up to four workers, so an SSD is used well without turning the build into a CPU or disk hog.
- Measured on the project benchmark: a 20,201-entry fixture build fell from 355 ms to 105 ms, and a broad 500,000-entry name query from 21 ms to 7 ms.
- The common name-sorted query walks a maintained name order instead of sorting every match, and page sorting selects only the rows it needs.
- Memory stays bounded by the existing entry and name-pool caps, and the benchmark records its memory use.

## Selection and export

- Check rows with the checkbox, ctrl/cmd-click or shift-click, or from the keyboard with Space, Shift+Arrow, Ctrl+A and Escape.
- The selection bar shows the checked count and size and can copy their paths or export just those rows.
- CSV and TXT export use the native save dialog. The footer exports every matching row of the current query and sort order, even when no row is checked.
- The backend validates the absolute `.csv`/`.txt` destination and writing one export is capped at 1,000,000 rows. CSV quoting handles commas, quotes and newlines.

## Compatibility and trust

- Version 0.2.3 updates the application, installers, PE metadata and About panel together.
- Upgrading preserves `%APPDATA%\dev.cyron.uninstaller` (history, cleanup log and quarantine). Deleting `search-index.bin` is safe; Search rebuilds it.
- The Windows binaries are still Authenticode-unsigned. Windows may show an unknown-publisher or SmartScreen warning. The Tauri updater signature proves that an update came from this project's update key; it is separate from Authenticode public trust.
- Verify downloads against the release's `SHA256SUMS.txt`.
