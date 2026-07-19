# Phase 8 Fault Policy Closeout

- Clean source commit: `1f3134524e86ceed2b8ba1369084dfa83d0fb7de`
- Generated timestamp: `2026-07-19T10:00:00+02:00`
- Platform: `trust-builder-linux-x86_64`

## Contract

Runtime anomaly stimuli remain ordinary inputs or exact scanner-bound test and
external harness facts. The metadata gate rejects production Cargo features and
public runtime symbols that expose fault-hook vocabulary. No production fault
hook, runtime behavior, suite, approved proof producer, or CI route changed.

The source guard is intentionally a design-review boundary, not a claim that
lexical inspection can prove the absence of every semantically disguised hook.
Any production hook requires an explicit reviewed contract change.

## Report Rebind

All affected reports were generated independently from the clean source commit.
Their JSON SHA-256 digests are:

| Report | JSON SHA-256 |
| --- | --- |
| P2 test class | `941d5a76ac5ff57e978d7b99a1b19f5613390fe7d4784314ee14c9ce3eaa6fce` |
| P2 coverage matrix | `a0d1fe2fbb183c4ee2a37ff1fb3b1e1ad10a5206b4b8f9ef519396e5f430e651` |
| P2 malformed input | `585fa6c80afc8001e667b42da0f40eeb0faf3b2d50801582d0585a62adf8b339` |
| P2 unmapped debt | `905c398dd041b3b4fdb51f703344d9b2342746e81d47fd44871c3baf6911601a` |
| P2A refactor | `4f792a35ec027b4e3d76c04d93bcad1159eeeede0bca042665acca6255b92049` |
| P4 invariant seeds | `662a3b05866118fc450f5bbcb150e2316e4c948ef10966e3bce29c6da627401b` |
| P4A specification | `d444d945f7e526c19a184a66040f124566f9a7a60c2c20bc1d3deaf53163b749` |
| P6 requirement/oracle | `1c6188c3b587572eaf3a59cbfa86db46efb59e9ddf405e492c19f18e14c88c24` |
| P7 conformance | `e4bd8f30f30902df66f24765ee19398d90336ebe41833695461c83cb4cbf9386` |
| P8 runtime anomaly | `11d1d9850dfd7d9a9cb1002b3f5079d0318634cacbc181ae2cc466dfec2c381f` |
| P9 fuzz | `fe7a7a527d047d4087b2a7b45caaf5a4e2ee24101a77d026b4a7a5fb6fc7eec0` |
| P10 mutation | `05fb90d11aaf664ed81ba809c0f719ed0fce130e16ebf15e605763203a2d8487` |
| P1A sources | `dd4fb17774414261af6967828996d309a982afca217ff08684adc9d1b0147a15` |
| P12 workflow/UI | `dc09e1adf8e6a39fe494d386e09b8d185a26e6e7e7eac3a29ebeb18a56ce09a8` |
| P13 release | `a32c52bbb4c4acdaec99c66e936b31f2963abbc5a382cc408a7f9efaf32b3bea` |
| P11 hardware | `65b4a2e070574f8c78987accdd1fbac470ef083c219243dedde12bd41782772e` |

The rebind also corrected three cataloged LSP facts whose exhaustive denominator
rows still carried their prior nonmapping disposition. The resulting partition
is 258 catalog-mapped plus 3,778 reviewed nonmapping facts, with zero unreviewed
facts.

## Final Validation

The committed evidence was validated from the isolated trust-builder worktree
at `0ef3db947b0b7de55570425cdd69dc81480855a8`:

- all 16 rebound report pairs passed their at-rest validators;
- the four refreshed census and open-row regression tripwires passed (4/4 in
  92.145 seconds);
- `just fmt`, `just clippy`, and `just test-all` all exited zero; and
- both metadata entry points validated 846 records, with `git diff --check`
  clean.

The earlier `just verification-veryquick` run had 927 passing tests and four
stale count fixtures. Those four fixtures were corrected and rerun directly;
the full 40-minute Python suite was not repeated after that test-only baseline
refresh.
