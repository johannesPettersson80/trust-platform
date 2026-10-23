from pathlib import Path
import json
path=Path('xtask/config/full_map_policy.json')
data=json.loads(path.read_text())
source='firmware/trust-nucleo-f401re/src/main.rs'
lines=Path(source).read_text().splitlines()
line=next(i for i,s in enumerate(lines,1) if s.startswith('unsafe fn HardFault('))
def visit(value):
    if isinstance(value,dict):
        if value.get('path') == source and 'line' in value:
            value['line']=line
        for item in value.values(): visit(item)
    elif isinstance(value,list):
        for item in value: visit(item)
visit(data)
path.write_text(json.dumps(data,indent=2)+'\n')
print('Existing HardFault unsafe-site location rebound to formatted line',line)
