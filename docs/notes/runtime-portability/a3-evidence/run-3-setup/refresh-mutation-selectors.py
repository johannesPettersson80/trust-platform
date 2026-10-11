from pathlib import Path
import hashlib, json, re, subprocess, os, sys
sys.path.insert(0, str(Path.cwd()))
from scripts.verification.mutation_execution import discover_mutants, select_generated_mutant
import tomllib
p=Path('verification/mutation-program.toml')
s=p.read_text()
program=tomllib.loads(s)
for shard in program['shards']:
    for mutation in shard['mutations']:
        source=Path(mutation['source_file'])
        candidates=discover_mutants(source.resolve(),Path.cwd(),os.environ.copy())
        selector = {key: mutation[key] for key in ('function', 'genre', 'replacement', 'selector_name')}
        # Whole-function substitutions have a unique semantic identity; formatting
        # may move their recorded position. Expression mutations need the exact site.
        if mutation['genre'] == 'FnValue':
            selector.pop('selector_name')
        selected = select_generated_mutant(candidates, selector)
        marker='id = "'+mutation['id']+'"'
        start=s.index(marker)
        end=s.find('\n[[',start)
        if end<0:end=len(s)
        block=s[start:end]
        block=re.sub(r'source_digest = "[^"]+"','source_digest = "sha256:'+hashlib.sha256(source.read_bytes()).hexdigest()+'"',block)
        block=re.sub(r'selector_name = "[^"]+"',lambda _: 'selector_name = '+json.dumps(selected['name']),block)
        s=s[:start]+block+s[end:]
p.write_text(s)
