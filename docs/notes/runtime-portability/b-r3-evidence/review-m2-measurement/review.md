# Independent B-R3 M2 measurement reconciliation

The retained evidence supports the link/size claim. Both measurement ledger steps (canonical firmware link and footprint report) exited 0. No product, test, compiler, linker or validator was run by this review. Read-only map/JSON arithmetic and static ELF symbols/program headers were inspected; source remains frozen during the software batch.

## Capacity arithmetic

- Upper firmware span: 441,920 B out of 458,752 B; free 16,832 B. This exceeds the 16,384 B floor by exactly **448 B**. It is narrow bring-up headroom, not capacity for unspecified future features.
- Sector 0: span 15,648 B, free 736 B; unchanged from M1. Application sector 1 and checkpoint sectors 2/3 remain outside firmware load ranges.
- M1 upper span 454,976 B minus M2 span 441,920 B =13,056 B saved. This consists of .text−12,416 B, upper .rodata−628 B, and upper padding−12 B.
- Map occupied section load falls 470,584→457,540 B (−13,044 B), while sector 0 placement stays fixed. No relocation is credited as an occupied-section reduction.
- ELF __firmware_load_end is 0x0807be40; sector 0 start/end symbols are 0x080001a0/0x08003d20. PT_LOAD has read-only sector 0, executable upper .text, read-only upper .rodata, and a zero-file-size RAM BSS segment. The initial entry is 0x08010001. Actual ELF loaded segments include the 12-byte gap after vectors, whereas the map section-byte total excludes that gap. The upper load-end includes the final 16-byte alignment reserve beyond rodata. Those differences are intentional accounting distinctions, not contradictory totals.

## Disjoint report family deltas

| Family | M1 B | M2 B | Delta B |
|---|---:|---:|---:|
| adapter | 2,154 | 2,154 | +0 |
| allocator | 304 | 304 | +0 |
| btree | 19,244 | 6,506 | -12,738 |
| decode | 16,402 | 16,426 | +24 |
| dispatch | 55,732 | 55,706 | -26 |
| encode | 0 | 0 | +0 |
| engine | 41,532 | 44,368 | +2,836 |
| firmware | 16,220 | 16,220 | +0 |
| libm | 17,068 | 17,068 | +0 |
| other | 129,086 | 126,656 | -2,430 |
| preparation | 34,984 | 35,224 | +240 |
| sort | 562 | 562 | +0 |
| stdlib | 24,534 | 24,552 | +18 |
| validator | 57,378 | 57,058 | -320 |

The existing report classifies demangled named text conservatively; inline code lives in its caller and unnamed text/alignment remains residual. Family deltas are measurements of the combined candidate, not isolated patch savings.

## Strong symbol evidence

The BTree family drops 12,738 B. All measured do_merge (2,684 B), remove_leaf_kv (1,722 B), bulk_steal_right (1,640 B), bulk_steal_left (1,620 B), choose_parent_kv (524 B), merge_tracking_child_edge (296 B) and map remove (220 B) groups disappear. Vacant-entry insertion (988 B) also disappears and other insertion/split groups shrink. This matches the selected deletion-heavy identity owners; prepared metadata still leaves 6,506 B of BTree code.

Engine text grows 2,836 B because compact storage needs its own search, growth accounting, bounded reservation and rollback paths. For example reserve_instance grows 752 B, OwnedInstances::push grows 396 B and its demand helper adds 296 B. remove_owned_instances_unchecked shrinks 748 B and LifecycleMarks::insert shrinks 360 B. Combined BTree-plus-engine change is therefore−9,902 B, not−12,738 B net collection savings. Other cross-family/inlined effects cannot be separated from these two builds.

The integer arithmetic body shrinks 3,940→1,640 B (−2,300 B), and the 278-byte __floattidf helper is absent. Scalar coerce shrinks 28 B. Those observations are consistent with native-width checked arithmetic and direct numeric conversion, which were independently source-reviewed to preserve domains and fault precedence. They do not prove behavior; native differential tests in the running batch remain the behavior evidence.

Removing the initializer inline experiment mainly redistributes code: the 7,864-byte nested construction closure disappears while a 7,816-byte validate_initializers symbol returns; validate_construction shrinks 212 B. The entire validator family falls 320 B. It would be incorrect to claim the disappearing closure alone saved 7.8KiB or removed validation.

No functionality removal, profile reduction, overflow-check change or relaxed admission is present in the reviewed candidate. Source review supports that architectural statement; the retained build config still pins 1.95.0, cortex-m4, safe ICF and checked-in linker scripts/remaps. Passing the full software batch, native ELF inspector and conditional physical board run are separate gates; this measurement review does not close them.
