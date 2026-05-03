---
name: set-tab-color
description: Set the color of the Warp tab a program is running in by emitting an OSC 1337 ; SetTabColor escape sequence. Use when the user asks to change, set, clear, or reset a tab's color from a script, command, or program — including coloring background tabs that are not currently focused.
---

# set-tab-color

Warp recognizes an OSC 1337 escape sequence for setting the tab color of the tab the program is running in. This is the programmatic equivalent of the `/set-tab-color` slash command and the right-click menu, but works from any script and targets the **source** tab (not just the currently active tab) — so a long-running command in a background tab can recolor itself.

## Sequence

```
OSC 1337 ; SetTabColor=<value> ST
```

- `OSC` is `\e]` (ESC `]`, byte sequence `0x1b 0x5d`).
- `ST` is the string terminator: either `\e\\` (ESC `\`) or `\a` (BEL). Both work.
- `<value>` is one of the accepted tokens below (case-insensitive, surrounding whitespace ignored).

## Accepted values

Six named ANSI colors:

- `red`
- `green`
- `yellow`
- `blue`
- `magenta`
- `cyan`

Two reset tokens — these mean different things:

- `none` or `clear` — explicitly remove the color, suppressing any directory-based default Warp would otherwise apply.
- `default` or `reset` — revert to Warp's default behavior (the directory-based color, if one is configured).

Anything else (other ANSI names like `black`/`white`/bright variants, hex codes, `rgb:` strings) is silently ignored. There is no error reply on the PTY.

## Examples

Bash / zsh:

```bash
printf '\e]1337;SetTabColor=red\a'
printf '\e]1337;SetTabColor=blue\e\\'      # ST form, equivalent
printf '\e]1337;SetTabColor=none\a'        # clear, suppress directory default
printf '\e]1337;SetTabColor=default\a'     # restore directory default
```

Fish:

```fish
printf '\e]1337;SetTabColor=green\a'
```

Python:

```python
import sys
sys.stdout.write('\x1b]1337;SetTabColor=magenta\x07')
sys.stdout.flush()
```

Node.js:

```js
process.stdout.write('\x1b]1337;SetTabColor=cyan\x07');
```

## When to use which token

- A script wants to mark its tab during a long-running task and then restore Warp's behavior afterward → use a color, then `default` (or `reset`) when done. The directory-based color, if any, comes back.
- A script wants to suppress the directory-based color entirely (e.g. show "no color" while running) → use `none` (or `clear`).
- The two are not interchangeable: `none` keeps the override active in the cleared state; `default` removes the override.

## Notes for AI agents

- This sequence is Warp-specific. Other terminals will either ignore it or, if they implement iTerm2's OSC 1337 image protocol, treat it as an unknown sub-command and skip it. Safe to emit unconditionally, but you can gate on `TERM_PROGRAM=WarpTerminal` if you want zero noise elsewhere.
- The sequence does not produce visible output — `printf` writes the escape directly to the controlling terminal.
- The user can still override the color via the right-click menu or `/set-tab-color`; this is a hint, not a lock.
- The slash command `/set-tab-color` exists for users typing into the prompt and only exposes the named colors and `none`. The OSC has the additional `default`/`reset` tokens because programs need finer control over restoration.
