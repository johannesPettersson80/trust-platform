#!/usr/bin/env bash
# Copy the repository's saved STBC containers into the local seed corpora of the
# bytecode_admission and bytecode_reprepare fuzz targets.
#
# fuzz/.gitignore ignores corpus/, so the copies stay untracked. Each copy is named after its
# repository path with "/" replaced by "__", which keeps files with the same basename apart.
# Rerunning the script overwrites the same files. The corpus and artifact directories are pruned
# from the search, so earlier output is never copied back in as a seed.
set -euo pipefail

fuzz_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd -- "$fuzz_dir/.." && pwd)"
corpora=(bytecode_admission bytecode_reprepare)

seeds=()
while IFS= read -r -d '' seed; do
  seeds+=("$seed")
done < <(
  cd -- "$repo_dir" && find . \
    \( -path ./target -o -path ./fuzz/target -o -path ./fuzz/corpus -o -path ./fuzz/artifacts \
      -o -path ./fuzz/coverage -o -name node_modules -o -name .git \) -prune \
    -o -type f -name '*.stbc' -print0
)

if ((${#seeds[@]} == 0)); then
  echo "seeds.sh: no *.stbc files under $repo_dir" >&2
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
