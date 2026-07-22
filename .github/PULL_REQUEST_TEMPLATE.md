## Summary

<!-- What and why -->

## Checklist

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked -- -D warnings`
- [ ] `cargo test --locked`
- [ ] Commits are DCO signed-off (`git commit -s`)
- [ ] If `--json` output changed, `schemas/v1/` updated in this PR
- [ ] No tokens, `~/.ethira` dumps, or full inventory `raw_json` in the PR
