/* F401RE sector 0: vectors and immutable firmware data; sector 1: STBC bundle.
 * Sectors 2..3: independent future checkpoints; sectors 4..7: firmware. */
MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 512K
  RAM : ORIGIN = 0x20000000, LENGTH = 96K
}
/* Select complete input sections before link.x's general .rodata wildcard.
 * The canonical build normalizes source paths; never spill into sector 1. */
SECTIONS
{
  .firmware_rodata ALIGN(ADDR(.vector_table) + SIZEOF(.vector_table), 32) : ALIGN(32)
  {
    __firmware_rodata_start = .;
    /* Compact decimal codecs replace the old core float tables. Use their
     * vacated space for the measured merged immutable strings instead. */
    *(.rodata.str1.1)
    *(.rodata.cst4 .rodata.cst8 .rodata.cst16 .rodata.cst32)
    *(.rodata.*stdlib*parameters*)
    /* Named complete descriptor arrays and immutable decimal/lookup tables.
     * Source namespaces are shortened by the canonical build; bounds still fail
     * closed if a compiler or registry change exceeds the measured capacity. */
    __firmware_descriptor_tables_start = .;
    *(.rodata.*stdlib*FUNCTIONS* .rodata.*stdlib*FAMILIES*)
    __firmware_descriptor_tables_end = .;
    *(.rodata.*ryu*DIGIT_TABLE* .rodata.*ryu*POW5*)
    *(.rodata..Lswitch.table.*)
    . = ALIGN(32);
    /* Two immutable 256-byte tables measured in the pinned full-function image. */
    __firmware_fixed_tables_start = .;
    *(.rodata.*libm*sqrt*RSQRT_TAB*)
    *(.rodata.*core*unicode*white_space*WHITESPACE_MAP*)
    __firmware_fixed_tables_end = .;
    __firmware_rodata_end = .;
  } > FLASH
} INSERT AFTER .vector_table;

_stext = 0x08010000;
_stack_start = 0x20018000;
_stack_end = 0x20014000;
