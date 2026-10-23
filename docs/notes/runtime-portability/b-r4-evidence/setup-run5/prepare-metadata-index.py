from pathlib import Path
import os,shutil,subprocess
out=Path('/home/johannes/.cache/trust-portability-b-evidence/b-r4-run5/review-index')
assert not out.exists(), 'temporary index already exists'
source=subprocess.check_output(['git','rev-parse','--git-path','index'],text=True).strip()
shutil.copy2(source,out)
env=dict(os.environ,GIT_INDEX_FILE=str(out))
subprocess.run(['git','add','-A'],env=env,check=True)
print('Candidate registered in an isolated mutation/metadata review index; real index unchanged; no commit')
