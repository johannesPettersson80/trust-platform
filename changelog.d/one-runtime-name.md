### Fixed

- A runtime that keeps debugging disabled keeps the name Connect gave it, such as `Cpu`, in the status bar, in the side
  panel's Target and in HMI Live. Before, the status bar and the side panel named it by its address
  (`127.0.0.1:<port>`) and HMI Live by its whole endpoint (`tcp://127.0.0.1:<port>`). HMI Live now names every runtime
  as the status bar does, the project's own runtime as `This project's runtime` (specification 25 §6.9, specification
  39 §11 item 6).
