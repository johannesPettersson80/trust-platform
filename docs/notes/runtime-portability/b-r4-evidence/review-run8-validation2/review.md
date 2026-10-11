# Run8 explicit supplemental validation review

Accepted, source-only independent review by /root/br1_static_registry. No tests, formatter, link or hardware commands run by reviewer.

First run8 native attempt is a retained failure before tests: unchanged-source parity included full_map_policy.json, but the already-frozen preparation refreshed the existing HardFault location from line133 to135 after extern crate alloc added two lines. Replacing only that location in current JSON reproduces prior ee40fb1f57ba339ab9c62b62d9ebe5be21b9ea00bdd9f068dd4a5c3bcad42b15 byte-for-byte. Current JSON 869b36796deb0f8151d847903075272e6b68c662eb998479c12dfad1a8ff3dc3 is already in the reviewed freeze. No runtime source correction is needed.

Supplemental setup-run8-validation2 changes paths to an explicit validation2 evidence directory; existing run8 source freeze/measurement are reused without reformatting, linking or remeasurement. A new one-use validation marker prevents hidden reruns. The measured-review marker binds the parent frozen manifest. Three changed source hashes pin main.rs, runner.rs and current policy; 169 unchanged-source hashes retain the prior runtime/host/lock/artifact proof. Both lists match current files and all applicable frozen records. The independent architecture gate reruns current policy. Remaining command allocation equals the reviewed focused run8 batch, with independent failures retained, dependent diagrams unrun after prerequisite failure, and no board launch in this script. Format is check-only.

Physical replay requires the successful validation2 ledger plus reviewed parent measurement/ELF, not the failed initial run8 validation ledger. Root must bind those exact successful evidence paths before board launch. No hardware acceptance is granted here.
