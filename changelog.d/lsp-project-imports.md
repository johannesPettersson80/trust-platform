### Fixed

- The language server no longer gives every file of a project a copy of every other file's declarations: a file's
  analysis holds the declarations it names, with those they need in turn (specification 14 §10.2). On a 767-file
  workspace its memory halved (about 3.5 GB to 1.8 GB) and its work after a burst of edits took a third less time;
  a 100-file project with every file open needs 170 MB instead of 231 MB. Completion, the learner's suggestions and
  rename's conflict check still see every declaration of the project.
