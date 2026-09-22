"""Install/remove only this bridge's entries; preserve other Codex work."""
import contextlib
import fcntl
import json
import os
from pathlib import Path
import plistlib
import shlex
import subprocess
import sys
import time
import tomllib

BEGIN='# BEGIN centaur-context bridge (managed)'
END='# END centaur-context bridge (managed)'
NAME='centaur_context'
LABEL='com.centaur.context.codex'


def atomic(path, data):
    path.parent.mkdir(parents=True,exist_ok=True)
    temp=path.with_name(path.name+'.centaur-tmp')
    with temp.open('wb') as f:
        os.chmod(temp,0o600);f.write(data);f.flush();os.fsync(f.fileno())
    os.replace(temp,path)


def install(settings, codex_home, *, remove=False, launchd=False):
    codex_home=codex_home.expanduser().resolve();codex_home.mkdir(parents=True,exist_ok=True)
    with (settings.root/'install.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        manifest_path=settings.root/'installation.json'
        previous=json.loads(manifest_path.read_text()) if manifest_path.exists() else None
        if previous and previous['codex_home']!=str(codex_home):raise ValueError('Existing installation belongs to another Codex home')
        config_path=codex_home/'config.toml';hooks_path=codex_home/'hooks.json'
        config=config_path.read_text() if config_path.exists() else ''
        hooks=json.loads(hooks_path.read_text()) if hooks_path.exists() else {'hooks':{}}
        original_config=config;original_hooks=json.dumps(hooks,sort_keys=True)
        if (BEGIN in config)!=(END in config):raise ValueError('Incomplete owned configuration block; inspect before changing')
        if BEGIN in config:
            if not previous:raise ValueError('Owned configuration has no installation manifest')
            start=config.index(BEGIN);stop=config.index(END,start)+len(END)
            if config[start:stop]!=previous['config_block']:raise ValueError('Owned configuration was edited; reconcile before changing')
            config=config[:start]+config[stop:]
        elif NAME in tomllib.loads(config).get('mcp_servers',{}):
            raise ValueError('Existing MCP server name is owned by another configuration')
        if previous:
            for event,group in previous['hook_groups'].items():
                groups=hooks.get('hooks',{}).get(event,[])
                if group in groups:groups.remove(group)
                elif not remove:raise ValueError('Owned hook was changed or removed; reconcile before reinstalling')
        python=str(Path(sys.executable).absolute())
        args=['-m','codex_context.bridge','--config',str(settings.path)]
        block=BEGIN+'\n[mcp_servers.'+NAME+']\ncommand='+json.dumps(python)+'\nargs='+json.dumps(args+['mcp'])+'\nstartup_timeout_sec=15\ntool_timeout_sec=30\n'+END
        groups={}
        if not remove:
            config=config.rstrip()+'\n\n'+block+'\n'
            command=shlex.join([python,*args,'hook'])
            for event in ('SessionStart','UserPromptSubmit','Stop','Interrupt','SessionEnd'):
                group={'hooks':[{'type':'command','command':command,'timeout':3 if event in ('Interrupt','SessionEnd') else 10,'statusMessage':'Saving Centaur Context'}]}
                groups[event]=group;hooks.setdefault('hooks',{}).setdefault(event,[]).append(group)
        tomllib.loads(config)
        backup=settings.root/('install-backup-'+str(time.time_ns()));backup.mkdir(mode=0o700)
        for path in (config_path,hooks_path):
            if path.exists():atomic(backup/path.name,path.read_bytes())
        # Optimistic checks avoid clobbering an edit made while preparing this update.
        if (config_path.read_text() if config_path.exists() else '')!=original_config:raise ValueError('Codex config changed concurrently; retry')
        if json.dumps(json.loads(hooks_path.read_text()) if hooks_path.exists() else {'hooks':{}},sort_keys=True)!=original_hooks:raise ValueError('Codex hooks changed concurrently; retry')
        atomic(config_path,config.encode());atomic(hooks_path,(json.dumps(hooks,indent=2)+'\n').encode())
        if launchd:
            if sys.platform!='darwin':raise ValueError('launchd installation requires macOS')
            plist=Path.home()/'Library/LaunchAgents'/f'{LABEL}.plist'
            domain=f'gui/{os.getuid()}'
            if plist.exists():
                existing=plistlib.loads(plist.read_bytes())
                if existing.get('Label')!=LABEL or (previous and str(plist)!=previous.get('launchd_path')):
                    raise ValueError('Existing launch agent is not owned by this installation')
                subprocess.run(['launchctl','bootout',domain,str(plist)],capture_output=True)
            if remove:
                if plist.exists():plist.unlink()
            else:
                atomic(plist,plistlib.dumps({'Label':LABEL,'ProgramArguments':[python,*args,'flush'],'RunAtLoad':True,'StartInterval':30,'StandardOutPath':'/dev/null','StandardErrorPath':'/dev/null'}))
                subprocess.run(['launchctl','bootstrap',domain,str(plist)],check=True,capture_output=True)
        if remove:
            if manifest_path.exists():manifest_path.unlink()
        else:
            atomic(manifest_path,json.dumps({'codex_home':str(codex_home),'config_block':block,'hook_groups':groups,'launchd_path':str(Path.home()/'Library/LaunchAgents'/f'{LABEL}.plist') if launchd else None},indent=2).encode())
        return {'installed':not remove,'backup':str(backup),'hook_trust':'Review exact hook definitions in Codex /hooks before they execute.' if not remove else 'Owned hooks removed; stored history and unsent queue retained.'}
