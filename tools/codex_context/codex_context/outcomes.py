"""Read-only Git evidence collector; no model-supplied completion claims."""
import hashlib
import subprocess
import time
from .privacy import redact


def git(cwd, *args):
    return subprocess.run(['git','--no-replace-objects','-C',cwd,*args],capture_output=True,check=True,timeout=3).stdout


def checkpoint(db, session, turn, cwd):
    try:
        head=git(cwd,'rev-parse','--verify','HEAD').decode().strip()
    except subprocess.CalledProcessError:
        return  # An unborn repository has no defensible pre-turn baseline.
    db.execute('INSERT OR IGNORE INTO git_checkpoints VALUES(?,?,?,?,?,NULL)',(session,turn,cwd,head,time.time()))


def observe(db, session, turn, cwd):
    row=db.execute('SELECT * FROM git_checkpoints WHERE session_id=? AND turn_id=?',(session,turn)).fetchone()
    if row is None or row['observed'] is not None or row['cwd']!=cwd:return None
    head=git(cwd,'rev-parse','--verify','HEAD').decode().strip()
    if head==row['head']:return None
    if git(cwd,'rev-parse',row['head']+'^{tree}')==git(cwd,'rev-parse',head+'^{tree}'):
        return None  # Empty commits/repeated bookkeeping are not meaningful code outcomes.
    # Only first-parent additions. A rebase or checkout is not a completed-work claim.
    ids=git(cwd,'rev-list','--first-parent','--max-count=11',row['head']+'..'+head).decode().splitlines()[::-1]
    if not ids or len(ids)>10:return None
    commits=[];parent=row['head']
    for oid in ids:
        size=int(git(cwd,'cat-file','-s',oid))
        if size>16_384:return None
        raw=git(cwd,'cat-file','commit',oid)
        digest=hashlib.sha1 if len(oid)==40 else hashlib.sha256
        if digest(b'commit '+str(len(raw)).encode()+b'\0'+raw).hexdigest()!=oid:return None
        text=raw.decode('utf-8')
        if redact(text) != text:return None  # Redaction would invalidate the proof hash.
        headers=text.split('\n\n',1)[0].splitlines()
        parents=[x[7:] for x in headers if x.startswith('parent ')]
        if not parents or parents[0]!=parent:return None
        stamp=int(next(x for x in headers if x.startswith('committer ')).rsplit(' ',2)[1])
        if stamp<row['started']-2 or stamp>time.time()+5:return None
        # Imported old commits and unrelated checkouts are excluded. Still describe
        # only observed worktree change, not this agent's authorship or test result.
        commits.append({'oid':oid,'raw':text});parent=oid
    db.execute('UPDATE git_checkpoints SET observed=? WHERE session_id=? AND turn_id=?',(head,session,turn))
    return {'turn_id':turn,'baseline':row['head'],'commits':commits}
