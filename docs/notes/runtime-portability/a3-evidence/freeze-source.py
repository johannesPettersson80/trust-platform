from pathlib import Path
import hashlib, json, subprocess
paths=set(subprocess.check_output(['git','ls-files','--modified','--others','--exclude-standard','-z']).decode().split('\0'))-{''}
record={name:hashlib.sha256(Path(name).read_bytes()).hexdigest() if Path(name).is_file() else None for name in sorted(paths)}
out=Path('/home/johannes/.cache/trust-portability-a3-evidence/run-1')
(out/'formatted-source-manifest.json').write_text(json.dumps(record,indent=2)+'\n')
print(f'Frozen {len(record)} source/deletion records after formatting and digest refresh')
