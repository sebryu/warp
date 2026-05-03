# Discoverability ideas for OSC 1337 SetTabColor

Tracks alternative or complementary surfaces for making the OSC 1337 SetTabColor
escape sequence discoverable. The bundled skill at `SKILL.md` is the in-repo
deliverable; everything below is either out of scope for this repo, lower
priority, or a deliberate follow-up.

## Shipped

- **Bundled skill** (`resources/bundled/skills/set-tab-color/SKILL.md`).
  Auto-loaded by `app/src/ai/skills/skill_manager.rs::read_bundled_skills`.
  Reaches Warp's built-in AI agent immediately.

## Considered but not shipped here

### Public docs page on docs.warp.dev
The most durable surface — gets indexed by search engines and scraped into
future AI training data.

Docs are open-sourced at `warpdotdev/docs` (MIT, Astro/Starlight, content
under `src/content/docs/**/*.mdx`, fork-and-PR flow in `CONTRIBUTING.md`).
Likely landing spots:
- `src/content/docs/terminal/more-features/` — neighbors `notifications.mdx`
  (OSC 9 / OSC 777) and `full-screen-apps.mdx`, the closest prior art for
  escape-sequence docs.
- `src/content/docs/terminal/appearance/tabs-behavior.mdx` or
  `terminal/windows/tabs.mdx` — tab-related features.

Per `CONTRIBUTING.md`, file an issue first for new content, then a PR. Out
of scope for this repo but actionable in the docs repo.

### Shell helper function (bash/zsh/fish)
A `warp_set_tab_color <color>` function in the Warp shell bootstrap
(`app/assets/bundled/bootstrap/{bash,zsh}_body.sh`, `fish.sh`) would make the
feature discoverable to *any* AI with shell access via `type
warp_set_tab_color` or `command -v` — not just Warp's own agent.

Tradeoffs:
- Adds three implementations to keep in sync.
- Pollutes the function namespace in every Warp shell.
- No precedent for user-facing functions in those scripts; existing `warp_*`
  helpers are internal plumbing (`warp_send_hook_kv_pair`, `warp_precmd`).
- Would need a usage / `--help` mode and arg validation to be useful.

Worth doing if we decide we want the feature to be discoverable outside Warp's
own agent loop (e.g. for users running Claude Code or another CLI agent inside
a Warp tab).

### `warp` CLI subcommand
Something like `warp tab color red` that emits the OSC sequence to its
controlling TTY. AI tools reflexively try `<tool> --help`, so this gets free
discovery via standard tool introspection. Pairs naturally with a man page.
Requires touching the CLI crate; not free.

### Man page
Document the OSC sequence in a `warp(1)` or `warp-escape-sequences(7)` man
page installed alongside the binary. Discoverable via `man -k tab` /
`apropos`. Low traffic but the "correct Unix way".

### Terminfo / extended capability
Define a custom terminfo capability (e.g. `Stbc` for "set tab background
color") on the `warp` terminfo entry. Programs and libraries that consult
`tigetstr`/`infocmp` would see it. Niche but the standard mechanism for
"does this terminal support feature X".

### iTerm2 OSC 1337 alignment
OSC 1337 is iTerm2's namespace. Consider proposing `SetTabColor` upstream so
iTerm2 (and other iTerm-compatible emulators) implement the same sequence.
Reach: every script that already uses `OSC 1337 ; SetMark`,
`OSC 1337 ; ClearScrollback`, etc. would naturally reach for ours. Long
horizon, requires upstream engagement.

### Slash command help text
Verify `/set-tab-color` exposes a help line that mentions the OSC equivalent
("scripts can also emit OSC 1337 ; SetTabColor=…"). One sentence in the
existing slash-command description, near-zero cost.

### Companion package on npm / PyPI
A tiny `@warpdev/tab-color` (npm) and `warp-tab-color` (PyPI) package whose
sole job is to wrap `process.stdout.write(...)` / `sys.stdout.write(...)` for
the OSC sequence. Indexed by package registries and AI training data; users
would `npm install` it and AI assistants suggest it from the package name
alone. Low maintenance, high passive reach.

### Capability advertisement via DA / OSC query
A more ambitious idea: have Warp respond to a DA (Device Attributes) or
private-mode query so programs can probe "does this terminal support tab
color?" before emitting the sequence. Useful for libraries that want to
gate behavior on terminal capabilities rather than `TERM_PROGRAM` sniffing.

## Notes

- The skill in this directory is the immediate, in-Warp answer.
- Everything else is either cross-repo work or a wider-scope feature; track
  here so we don't re-derive the option space next time someone asks "how do
  we make this discoverable".
