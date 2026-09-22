import io
import json
from pathlib import Path
import sqlite3
import subprocess
import uuid

import pytest
from codex_context import bridge as b


@pytest.fixture
def setup(tmp_path):
    repo = tmp_path/'repo'; repo.mkdir()
    subprocess.run(['git','init','-q',str(repo)],check=True)
    transcripts=tmp_path/'sessions';transcripts.mkdir()
    state=tmp_path/'state';state.mkdir(mode=0o700)
    token=tmp_path/'capture-token';token.write_text('synthetic-capture-token-'+'x'*32);token.chmod(0o600)
    tools=tmp_path/'tool-token';tools.write_text('synthetic-tool-token-'+'x'*32);tools.chmod(0o600)
    cfg=tmp_path/'config.json'
    data={'version':1,'host_id':str(uuid.uuid4()),'state_dir':str(state),'transcript_root':str(transcripts),
      'targets':{'organization':{'url':'http://127.0.0.1:1','capture_token_file':str(token),'tool_token_file':str(tools)}},
      'repositories':{'project':{'path':str(repo),'target':'organization'}}}
    cfg.write_text(json.dumps(data))
    sid=str(uuid.uuid4());path=transcripts/'session.jsonl'
    append(path,{'type':'session_meta','payload':{'id':sid,'cli_version':'0.153.4','history_mode':'paginated','timestamp':'2026-09-22T01:00:00Z'}})
    return b.Settings(cfg),repo,sid,path


def append(path,record):
    with path.open('a') as f:f.write(json.dumps({'timestamp':'2026-09-22T01:00:00Z',**record})+'\n')


def item(path,sid,turn,kind,id,text):
    append(path,{'type':'event_msg','payload':{'type':'item_completed','thread_id':sid,'turn_id':turn,
      'item':{'type':kind,'id':id,'content':[{'type':'text' if kind=='UserMessage' else 'Text','text':text}]}}})


def event(repo,sid,path,name='SessionStart',turn=None):
    return {'session_id':sid,'transcript_path':str(path),'cwd':str(repo),'hook_event_name':name,**({'turn_id':turn} if turn else {})}


def pending(settings):
    with b.database(settings) as db:return [json.loads(r[0]) for r in db.execute('SELECT payload FROM batches ORDER BY rowid')]


def test_capture_excludes_history_instructions_reasoning_tools_and_other_threads(setup):
    s,repo,sid,path=setup;turn=str(uuid.uuid4())
    item(path,sid,turn,'UserMessage','old','Prior history must not be imported.')
    b.capture(s,event(repo,sid,path))
    for kind in ['Reasoning','CommandExecution','McpToolCall']:
        item(path,sid,turn,kind,kind,'DO NOT CAPTURE')
    append(path,{'type':'response_item','payload':{'type':'message','role':'developer','content':'DO NOT CAPTURE'}})
    item(path,str(uuid.uuid4()),turn,'UserMessage','forked','Inherited/other thread DO NOT CAPTURE')
    item(path,sid,turn,'UserMessage','new','I chose Friday.')
    item(path,sid,turn,'AgentMessage','reply','Understood.')
    append(path,{'type':'event_msg','payload':{'type':'task_complete','turn_id':turn}})
    b.capture(s,event(repo,sid,path,'Stop',turn))
    payload=pending(s)[1]
    assert [m['content'] for m in payload['messages']]==['I chose Friday.','Understood.']
    assert payload['finished_turn_id']==turn
    before=len(pending(s));b.capture(s,event(repo,sid,path,'Stop',turn));assert len(pending(s))==before


def test_partial_write_cursor_and_duplicate_hook(setup):
    s,repo,sid,path=setup;b.capture(s,event(repo,sid,path));turn=str(uuid.uuid4())
    prefix=path.stat().st_size
    with path.open('a') as f:f.write('{"type":"event_msg",')
    b.capture(s,event(repo,sid,path,'Stop',turn))
    with b.database(s) as db:assert db.execute('SELECT cursor FROM sessions').fetchone()[0]==prefix
    with path.open('a') as f:f.write('"payload":{"type":"task_complete","turn_id":"'+turn+'"}}\n')
    b.capture(s,event(repo,sid,path,'Stop',turn))
    assert pending(s)[-1]['finished_turn_id']==turn


def test_queue_full_rolls_back_cursor(setup,monkeypatch):
    s,repo,sid,path=setup;b.capture(s,event(repo,sid,path));turn=str(uuid.uuid4())
    with b.database(s) as db:cursor=db.execute('SELECT cursor FROM sessions').fetchone()[0]
    item(path,sid,turn,'UserMessage','a','An important decision.')
    monkeypatch.setattr(b,'MAX_QUEUE_BYTES',1)
    with pytest.raises(ValueError,match='queue is full'):b.capture(s,event(repo,sid,path,'Stop',turn))
    with b.database(s) as db:assert db.execute('SELECT cursor FROM sessions').fetchone()[0]==cursor


def test_worktree_identity_and_immutable_target(setup,tmp_path):
    s,repo,sid,path=setup
    subprocess.run(['git','-C',str(repo),'-c','user.name=Test','-c','user.email=test@example.invalid','commit','--allow-empty','-qm','initial'],check=True)
    work=tmp_path/'worktree';subprocess.run(['git','-C',str(repo),'worktree','add','--detach',str(work)],capture_output=True,check=True)
    assert s.route(str(work),sid)==('project','organization')
    b.capture(s,event(repo,sid,path))
    s.data['targets']['personal']=dict(s.targets['organization'])
    s.repositories['project']['target']='personal'
    with pytest.raises(ValueError,match='immutable'):b.capture(s,event(repo,sid,path))


def test_rejects_unmapped_repo_and_transcript_replacement(setup,tmp_path):
    s,repo,sid,path=setup;b.capture(s,event(repo,sid,path))
    other=tmp_path/'other';other.mkdir();subprocess.run(['git','init','-q',str(other)],check=True)
    with pytest.raises(ValueError,match='unconfigured'):s.route(str(other),str(uuid.uuid4()))
    path.write_text('{}\n')
    with pytest.raises(ValueError,match='Unsupported'):b.capture(s,event(repo,sid,path))


def test_current_message_limit_and_credential_redaction(setup):
    s,repo,sid,path=setup;b.capture(s,event(repo,sid,path));turn=str(uuid.uuid4())
    secret='ghp_'+'a'*32
    item(path,sid,turn,'UserMessage','secret','Token '+secret)
    b.capture(s,event(repo,sid,path,'Stop',turn))
    assert secret not in b.canonical(pending(s))
    item(path,sid,turn,'UserMessage','long','x'*20_001)
    with pytest.raises(ValueError,match='not silently truncated'):b.capture(s,event(repo,sid,path))


def test_failed_delivery_retains_batch_and_success_acks_exact_batch(setup,monkeypatch):
    s,repo,sid,path=setup;b.capture(s,event(repo,sid,path))
    monkeypatch.setattr(b.SessionClient,'_request',lambda *a,**k:(_ for _ in ()).throw(RuntimeError('unavailable')))
    assert b.flush(s)['delivered']==0
    assert len(pending(s))==1
    with b.database(s) as db,db:db.execute('UPDATE batches SET next_attempt=0')
    chat=str(uuid.uuid4())
    monkeypatch.setattr(b.SessionClient,'_request',lambda *a,**k:{'chat_object_id':chat,'curation_enabled':False})
    assert b.flush(s)['delivered']==1
    assert pending(s)==[]
    state=b.status(s)['sessions'][0]
    assert state['chat_id']==chat and state['last_receipt']['curation_enabled'] is False


def test_mcp_uses_runtime_session_metadata_and_only_three_tools(setup,monkeypatch):
    s,repo,sid,path=setup
    calls=[]
    monkeypatch.setattr(b,'flush',lambda *a,**k:{'delivered':0})
    with pytest.raises(ValueError,match='no trusted'):b.mcp_call(s,{'name':'context_search','arguments':{'query':'x'},'_meta':{'threadId':sid}})
    b.capture(s,event(repo,sid,path))
    monkeypatch.setattr(b.SessionClient,'context_search',lambda self,**args:calls.append((self.session['id'],self.session['target'],args)) or {'objects':[]})
    b.mcp_call(s,{'name':'context_search','arguments':{'query':'release'},'_meta':{'threadId':sid}})
    assert calls==[(sid,'organization',{'query':'release'})]
    with pytest.raises(ValueError):b.mcp_call(s,{'name':'capture','arguments':{},'_meta':{'threadId':sid}})
    assert [x['name'] for x in b.tool_schema()]==['context_search','context_read','context_apply']
    incoming=io.StringIO(json.dumps({'id':1,'method':'tools/list'})+'\n');out=io.StringIO();b.serve(s,incoming,out)
    assert len(json.loads(out.getvalue())['result']['tools'])==3


def test_tool_writes_wait_for_chat_and_provenance_is_not_model_selectable(setup,monkeypatch):
    s,repo,sid,path=setup;b.capture(s,event(repo,sid,path))
    monkeypatch.setattr(b,'flush',lambda *a,**k:{'delivered':0})
    params={'name':'context_apply','arguments':{},'_meta':{'threadId':sid}}
    with pytest.raises(ValueError,match='pending'):b.mcp_call(s,params)
    with b.database(s) as db,db:db.execute('UPDATE sessions SET chat_id=?',(str(uuid.uuid4()),))
    params['arguments']['chat_object_id']=str(uuid.uuid4())
    with pytest.raises(ValueError,match='supplies'):b.mcp_call(s,params)


def test_no_redirects_or_public_plaintext_endpoints(setup):
    s,*_=setup
    with pytest.raises(ValueError,match='redirect'):b.NoRedirect().redirect_request(None,None,None,None,None,None)
    d=s.data;d['targets']['organization']['url']='http://example.invalid';s.path.write_text(json.dumps(d))
    with pytest.raises(ValueError,match='HTTPS'):b.Settings(s.path)
