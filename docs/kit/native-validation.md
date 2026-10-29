# Native accessibility and interaction validation

Use the native acceptance harness from the repository root:

```sh
cargo +1.95.0 run -p wgpui-kit-harness --bin wgpui-native-acceptance
```

It covers actual windows, editing, actions, clipboard operations, file pickers
and secondary-window closure. Unlike the retired Story application, this command
uses the active WGPUI workspace. It does not imply that every OS permission or
accessibility bridge is available; record those prerequisites and any uncovered
operation explicitly. Linux native checks require the relevant X11/Wayland
desktop and portal services.

Locate controls by role, accessible name, value and focus. Re-read the tree after
every state-changing action because element indexes are snapshots. Verify actual
key events, selection, insertion, replacement, undo/redo, no-op edit behavior,
shortcuts, disabled/read-only state and privacy of secret values. Use disposable
text and file fixtures for clipboard and picker checks.

Check expected values at each step, not just process survival. Keep native input,
semantic snapshots and GPU screenshots as distinct evidence. Screenshots establish
clipping, colors, alignment and visibility; a correct semantic value alone cannot
prove the text is drawn. If coordinate input is necessary, explain the target
and capture the resulting state again.

Native menu reentry, multi-display recovery, physical trackpad input and closure
while a GPU handle survives need real window-system checks. Portable context
and offscreen tests do not replace them. Record the application/backend, input
sequence, observed values, screenshots where needed, command results and
remaining gaps. See [release status](../../STATUS.md) for acceptance boundaries.
