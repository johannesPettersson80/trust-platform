# Run6 state-phase linked result

Read-only independent inspection by `/root/br1_static_registry`. No compiler/test/formatter/linker/board operation; disassembly reads the existing ELF. Source freeze 6a8823e1b138ec193157ed08c0935da87548b377eddebeb2a7399c93d5f713b2; ELF 7c5aa689752d2bc199f56f9094b5cd8066a42d6b1a5403a62bf2aca7a41550fb. Upper free18592 bytes passes the flash criterion but is32 less than run5.

Measured allocation helper864 bytes (828 local+36 saved), which returns before recursive construction. Contrary to the intended benefit, instantiate_with_services grows from1824 to1976 bytes (1940 local+36 saved). Main fixture2488, evaluator968 and dispatcher1872 are unchanged. Therefore the identified deep-construction fixed-frame sum grows14400→14552 before leaf helpers/interrupts, exceeding14336 allowable usage for2KiB headroom.

This candidate is not stack-ready. Root and author were notified before launching native/physical validation. Source equivalence does not guarantee favorable stack placement: the new by-value Result<Self> boundary did not eliminate the caller's aggregate/result temporaries under the pinned compiler. Retain this unfavorable measurement honestly rather than crediting the phase split as a reduction. No software test or physical acceptance is inferred from link success.
