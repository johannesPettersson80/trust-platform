### Fixed

- OpenOT database persistence no longer counts a record that the program overwrote while the reader
  was reading it as rejected. Such a record is overwritten, as the OpenOT carriage contract orders,
  so the source's rejected count holds only records that fail validation (specification 33 §1). On
  ARM processors the reader's snapshot of the shared ring's control block is now always coherent.
