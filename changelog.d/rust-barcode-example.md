### Added
- The example `rust_barcode`, **Rust barcode reader** in the example gallery: a packing line's label
  check, where a function block written in Rust reads a GS1-128 label's text into its GTIN, batch and
  expiry date, with the GTIN's check digit and an expiry day of 00 read as the month's last, and the
  program accepts the box or names why not (expired, unreadable, another product). The block reports
  a label it cannot read with an error code and never stops the scan.
