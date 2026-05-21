# Trust-Twin Packaged Assets

Robot-cell packaged assets are externally sourced from these upstreams:

- Universal Robots ROS2 Description at commit
  `18dea90dedf24adea05d920d0d441e4523a41e50`:
  https://github.com/UniversalRobots/Universal_Robots_ROS2_Description/tree/18dea90dedf24adea05d920d0d441e4523a41e50
- RobotLocomotion/models at commit
  `5c942636d18013870403c17c8209558799122abd`:
  https://github.com/RobotLocomotion/models/tree/5c942636d18013870403c17c8209558799122abd

The imported asset manifest SHA256 is
`99dfc722f035472e684122fa8e77c3361242c103f7fe52dfacfaf845d1b470e5`.

Licenses, authors, and local paths:

- UR10 visual meshes under `ur10/visual/`: BSD-3-Clause. Original authors:
  Universal Robots ROS2 Description authors/Universal Robots A/S. The upstream
  COLLADA files were converted to glTF with the same mesh geometry and a Z_UP
  to glTF Y-up axis conversion.
- Schunk WSG-50 meshes under `schunk-wsg50/meshes/`: BSD-3-Clause. Author:
  Toyota Research Institute.
- Drake manipulation table mesh under `manipulation-station/assets/`:
  BSD-3-Clause. Author: Robot Locomotion Group at CSAIL.
- YCB object meshes under `ycb/meshes/`: CC-BY-4.0. Author/source: YCB Object
  and Model Set.

The manifest hash is the SHA256 of the sorted `<file-sha256>  <relative-path>`
list for the external `manipulation-station/`, `schunk-wsg50/`, `ur10/`, and
`ycb/` asset files.

Do not replace these files with generated geometry and keep the source URL,
license, original-author, and manifest hash synchronized with the robot-cell
topology/view metadata when the package changes.
