# Manual & Hardware Validation Checklist: DineInTakeOut

## Automated Checks
- [ ] All unit/integration tests (`cargo test`)
- [ ] Rustfmt/linting
- [ ] Coverage (`cargo llvm-cov`)

## PTY/Terminal Flows
- [ ] App boots in real terminal
- [ ] Keyboard navigation (arrows, enter, escape) succeeds
- [ ] Window resizing redraws UI correctly
- [ ] App cleanly quits
- [ ] No panics/crashes on launch/quit

## UI Rendering
- [ ] All screens reachable
- [ ] Bill/split/modals render as expected
- [ ] Notifications/messages visible
- [ ] Unicode (₹, €, ¥, etc) display as intended

## Printing
- [ ] Receipts print with valid totals, GST, offers
- [ ] Printer error handling visible (simulate CUPS down/misconfigured)
- [ ] No blocking/panics when printer unavailable

## Persistence/Database
- [ ] Orders persist, app restart recovers open/paid
- [ ] Corrupt, missing, or v0 data handled gracefully

## Display/Keyboard Hardware
- [ ] Works on target terminals (URxvt, GNOME, Alacritty, tty, etc)
- [ ] Layout functional on at least 80x24, 120x36, wide screens
- [ ] All arrow/function keys functional; Numpad works

## Optional/Advanced
- [ ] Non-ASCII names and very long item names supported
- [ ] Edge cases: empty data, very large bills, discount/excess tests
- [ ] Visual accessibility (contrast, color-blind modes if present)

For each release, complete and attach this checklist!
