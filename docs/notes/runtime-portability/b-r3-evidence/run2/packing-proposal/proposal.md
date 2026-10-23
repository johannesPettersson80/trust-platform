# Read-only remaining sector-0 packing proposal

No source edit, build, link, test or hardware execution. The batch source stays frozen.

There is no clearly named single immutable .rodata input section between 352 and 736 bytes in this map. The only single section in that interval is an anonymous compiler section of 480 bytes; do not select it by its unstable ordinal/hash.

The minimum straightforward alternative is two complete named immutable data tables:

| Symbol | Measured bytes | Input alignment | Current address |
| --- | ---: | ---: | --- |
| libm::math::generic::sqrt::RSQRT_TAB | 256 | 2 | 0x080768d0 |
| core::unicode::unicode_data::white_space::WHITESPACE_MAP | 256 | 1 | 0x08076f98 |

The libm source defines static [u16; 128], and the pinned Rust core source defines static [u8; 256]. Both are data-only tables, not code or behavior subsets. ELF .rodata has allocation/merge/string flags but no write or execute flag. These complete input tables are currently collected by the upper .rodata wildcard. Moving their whole sections before that wildcard preserves their contents and ordinary linker-resolved references.

Suggested narrow input selectors alongside the existing named-table selectors, before final ALIGN(32):

    *(.rodata.*libm*sqrt*RSQRT_TAB*)
    *(.rodata.*core*unicode*white_space*WHITESPACE_MAP*)

Measured sector-0 end is 0x08003d20, leaving 736 bytes. Appending 512 table bytes before final 32-byte alignment should move that end to 0x08003f20 and leave 224 bytes. Both sizes are multiples of 32 and their required alignments divide 32. Existing region and alignment assertions must remain unchanged.

The current upper free space is 16032 bytes, 352 short of the unchanged 16384-byte margin. Removing these 512 bytes would predict 16544 free, 160 above the margin. This is arithmetic on measured inputs, not new linked proof: only an authorized new link and unchanged ELF inspector can establish actual placement, any instruction/literal layout changes and final margin. No claim that this is release growth capacity.
