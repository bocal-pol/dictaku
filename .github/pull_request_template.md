## Summary

<!-- What does this PR do? Link the related issue: Closes #NNN -->



## Type of change

- [ ] Bug fix (non-breaking change that resolves an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (changes existing behaviour)
- [ ] Refactor (no functional change)
- [ ] Documentation update
- [ ] CI / tooling change

## Checklist

### Code quality
- [ ] `cargo clippy --all-targets -- -D warnings` passes with no warnings
- [ ] `cargo fmt --check` passes (code is formatted)
- [ ] `cargo test --lib` passes with no failures

### Testing
- [ ] Unit tests added or updated for the changed logic
- [ ] Manual test performed on Windows (describe scenario below)

### Documentation
- [ ] `CHANGELOG.md` updated under `## [Unreleased]` with a concise entry
- [ ] Public-facing doc or comments updated if API/behaviour changed
- [ ] `intake/data-dictionary.md` updated if `config.json` schema changed

### Security
- [ ] No secrets or credentials committed
- [ ] No hardcoded paths (use `directories` crate for user dirs)
- [ ] `cargo audit` run locally — no new vulnerabilities introduced

## Manual test scenario

<!-- Describe what you tested manually on Windows, e.g.:
1. Launched app, pressed Ctrl+Alt+D in Notepad
2. Spoke for 5 seconds, pressed Ctrl+Alt+D again
3. Text was injected correctly with proper punctuation
-->



## Screenshots / recordings

<!-- If the change affects the tray icon or any visible UI, attach a screenshot. -->
