# Run 8 software evidence reconciliation

Accepted: raw supplemental validation2 ledger contains exactly 17 required PASS and one advisory PASS, all exit 0. Raw test output records native adapter 6 + xtask 85 = 91, firmware library 2, provenance helper 4; zero failed/ignored/filtered. Mutation tooling 16 and mutation contracts 20 report OK. The unchanged-source checker reports 169 OK records. Current full_map_policy is separately pinned and passed architecture; diagrams/render drift, lint, formatting, pack and artifact checks passed.

The parent ledger retains the initial parity failure at 21:13:59Z, before tests. Supplemental validation begins 21:14:57Z with an explicit independent ledger. This is not one uninterrupted green attempt. No reformat/relink occurred in validation2; parent linked ELF/source identity remain those independently reviewed under review-run8-linked.

Core/portable/i686 and 20 hosted integration suite evidence remains run 7, not rerun in run 8. Historical full hosted unit/integration evidence remains run 4 where applicable. Those classes must not be relabeled as current new executions. The 169 unchanged file pins plus 3 changed input pins and repeated affected gates establish the described reuse chain.

Board approval file explicitly binds reviewed source a95bcff98b5d4a51e70acf7e29cf0113b585ac69bcfc39c4cc5fbac6b1e8337c, ELF a0e6ed2d7f725fd17746d30c229d02e5b0125c00c494015f3be40322e293ec1e, and successful validation2 ledger dfab36ffbd2613e27ff47ed62016338bfaa40386cc6b130a1510c78dd2ba0239. Hash recomputes. Hardware was running when this software reconciliation was written. Its later acceptance is recorded separately in review-run8-physical.

Independent reviewer /root/br1_static_registry read retained files only; no builds/tests/formatters/linkers/board commands were executed.
