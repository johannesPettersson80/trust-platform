#!/usr/bin/env bash
# Copy the program images under the repository into the local seed corpus of the image_admission
# fuzz target. `trust-runtime build` writes a project's image as program.tpi, and
# `trust-runtime build --target <device>` writes a device's image in the project's build folder.
#
# fuzz/.gitignore ignores corpus/, so the copies stay untracked. Each copy is named after its
# repository path with "/" replaced by "__", which keeps files with the same basename apart.
# Rerunning the script overwrites the same files. The corpus and artifact directories are pruned
# from the search, so earlier output is never copied back in as a seed.
set -euo pipefail

fuzz_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd -- "$fuzz_dir/.." && pwd)"
corpora=(image_admission)

seeds=()
while IFS= read -r -d '' seed; do
  seeds+=("$seed")
done < <(
  cd -- "$repo_dir" && find . \
    \( -path ./target -o -path ./fuzz/target -o -path ./fuzz/corpus -o -path ./fuzz/artifacts \
      -o -path ./fuzz/coverage -o -name node_modules -o -name .git \) -prune \
    -o -type f -name '*.tpi' -print0
)

if ((${#seeds[@]} == 0)); then
  echo "seeds.sh: no *.tpi program images under $repo_dir; build a project first" >&2
  exit 1
fi

for corpus in "${corpora[@]}"; do
  mkdir -p -- "$fuzz_dir/corpus/$corpus"
  for seed in "${seeds[@]}"; do
    name="${seed#./}"
    name="${name//\//__}"
    cp -- "$repo_dir/$seed" "$fuzz_dir/corpus/$corpus/$name"
  done
done

echo "copied ${#seeds[@]} seed(s) into ${corpora[*]} under $fuzz_dir/corpus/"
