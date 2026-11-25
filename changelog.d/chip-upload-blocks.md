### Fixed

- On an ESP32-C6, every block of an upload, an online change's image whatever its size, the
  transfer's part buffer and an install's container header, takes the program region. An image
  smaller than 32 KiB took the data heap for as long as the changed program ran, and the part
  buffer took it while the upload ran, leaving the chip's working memory that much smaller
  (specification 44 §10.3).
