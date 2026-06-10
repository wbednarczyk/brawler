---
name: brawler-release
description: Use when closing a Brawler milestone or release from the repository root. Covers version bump, Makefile-driven changelog generation, release validation, Radicle tracking, and the final release commit.
metadata:
  short-description: Close a Brawler release consistently
---

# Brawler Release

The canonical Brawler release workflow is repository-owned and agent-neutral:

```text
.agents/skills/brawler-release.md
```

When this skill is triggered, read and follow that file. Do not use older global `brawler-version-bump` instructions if they conflict with the repository-owned workflow.
