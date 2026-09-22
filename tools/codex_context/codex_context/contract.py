"""Package the core contract and derive MCP operation shapes from it."""
import json
from pathlib import Path


def resource(name, source):
    installed=Path(__file__).with_name(name)
    return (installed if installed.exists() else Path(__file__).resolve().parents[3]/source).read_text()


def document():
    return json.loads(resource('context-contract.json','contract/context-contract.json'))


def guidance():
    # Shared canonical prose; replace only CLI invocation instructions for MCP.
    return resource('context-agent-instructions.md','generated/context-agent-instructions.md').split('Call these commands directly;')[0]+\
        'Call the three MCP tools directly. Use their schemas for exact operation fields. Capture runs automatically; write other records only when the user requests it.'


def schemas():
    c=document();limits=c['limits'];tools=c['tools'];string={'type':'string'}
    array=lambda item:{'type':'array','items':item}
    obj=lambda props,req:{'type':'object','properties':props,'required':req,'additionalProperties':False}
    ref={'oneOf':[obj({'object_id':string},['object_id']),obj({'local_ref':string},['local_ref'])]}
    search=obj({'query':{**string,'maxLength':limits['search_query_characters']},'object_types':array({'type':'string','enum':list(c['object_types'])}),'limit':{'type':'integer','minimum':1,'maximum':limits['search_results']},'lexical_only':{'type':'boolean'},'task_filters':{'type':'object'}},tools['context_search']['input']['required'])
    read=obj({'object_ids':{**array(string),'minItems':1,'maxItems':limits['read_objects']},'include':array({'type':'string','enum':['connections','artifacts','events','messages']}),'artifact_windows':array({'type':'object'})},tools['context_read']['input']['required'])
    variants=[]
    for name,spec in tools['context_apply']['operations'].items():
        fields={'operation':{'type':'string','const':name}}
        for key in spec['required']+spec.get('optional',[]):
            fields[key]= {'type':'integer','minimum':1} if key=='expected_revision' else ref if key in ('source','target','object') else {'type':'object'} if key in ('provenance','fields','changes','metadata') else dict(string)
        variants.append(obj(fields,['operation',*spec['required']]))
    apply=obj({'contract_version':{'type':'string','const':c['contract_version']},'idempotency_key':string,'validate_only':{'type':'boolean'},'operations':{**array({'oneOf':variants}),'minItems':1,'maxItems':limits['apply_operations']}},tools['context_apply']['input']['required'])
    return search,read,apply
