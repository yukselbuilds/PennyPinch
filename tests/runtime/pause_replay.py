"""Bounded real pinned-engine pause replay. All model traffic stays on loopback."""
import base64,copy,ctypes,hashlib,http.client,importlib.util,json,os,re,socket,struct,subprocess,sys,threading,time,uuid
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];assert Path.cwd().resolve()==ROOT
BASE=ROOT/'work/proofs/pause';run=BASE/'runs'/str(uuid.uuid4());run.mkdir(parents=True)
runtime=ROOT/'work/proofs/packaging/artifacts/headroom-engine/runtime';python=runtime/'python.exe'
generation=uuid.uuid4().hex;secret=uuid.uuid4().hex;SENTINEL='headroom-local-nonfunctional-test'
module_path=ROOT/'work/proofs/packaging/headroom_runtime.py'
spec=importlib.util.spec_from_file_location('pause_owned_helper',module_path);helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
sources=[ROOT/'runtime/optimization_control.py',ROOT/'runtime/optimization-sources.json',ROOT/'runtime/cache_control_shim.py',Path(__file__),module_path,python]
package=runtime/'Lib/site-packages/headroom';sources += [package/p for p in json.loads((ROOT/'runtime/optimization-sources.json').read_text())]
fingerprint=lambda:{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sources}
before=fingerprint();captured={};lock=threading.Lock();entered=threading.Event();release=threading.Event()
def require(value,code):
 if not value:raise AssertionError(code)
def frame(event,payload):return ('event: '+event+'\ndata: '+json.dumps(payload)+'\n\n').encode()
def ws_read(stream):
 def exact(n):
  value=stream.read(n)
  if len(value)!=n:raise EOFError()
  return value
 first,size=exact(2);opcode=first&15;masked=bool(size&128);size&=127
 if size==126:size=struct.unpack('!H',exact(2))[0]
 elif size==127:size=struct.unpack('!Q',exact(8))[0]
 require(size<=2*1024*1024 and first&128,'bounded_complete_ws_frame')
 mask=exact(4) if masked else None;data=exact(size)
 if mask:data=bytes(v^mask[i%4] for i,v in enumerate(data))
 return opcode,data
def ws_write(stream,value,masked=False,opcode=1):
 data=json.dumps(value,separators=(',',':')).encode() if opcode==1 else value
 length=len(data);flag=128 if masked else 0
 header=bytes([128|opcode,flag|length]) if length<126 else bytes([128|opcode,flag|126])+struct.pack('!H',length) if length<65536 else bytes([128|opcode,flag|127])+struct.pack('!Q',length)
 if masked:
  mask=os.urandom(4);header+=mask;data=bytes(v^mask[i%4] for i,v in enumerate(data))
 stream.write(header+data);stream.flush()
ws_connections=[0]
class Handler(BaseHTTPRequestHandler):
 protocol_version='HTTP/1.1'
 def log_message(self,*args):pass
 def do_GET(self):
  require(self.path=='/v1/responses' and self.headers.get('Upgrade','').lower()=='websocket','fixture_ws_route')
  self.close_connection=True;self.connection.settimeout(45)
  accept=base64.b64encode(hashlib.sha1((self.headers['Sec-WebSocket-Key']+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
  self.send_response(101);self.send_header('Connection','Upgrade');self.send_header('Upgrade','websocket');self.send_header('Sec-WebSocket-Accept',accept);self.end_headers()
  with lock:ws_connections[0]+=1
  index=0
  try:
   while True:
    opcode,data=ws_read(self.rfile)
    if opcode==8:ws_write(self.wfile,data,opcode=8);return
    if opcode==9:ws_write(self.wfile,data,opcode=10);continue
    require(opcode==1,'fixture_text_frame');body=json.loads(data);require(body['type']=='response.create','fixture_create_frame');index+=1
    with lock:captured.setdefault('responses-ws-'+str(index),[]).append({'body':body,'auth':self.headers.get('Authorization')})
    response={'id':'resp-ws-'+str(index),'status':'in_progress','output':[],'model':body['model']}
    ws_write(self.wfile,{'type':'response.created','response':response})
    if index==2:entered.set();release.wait(15)
    ws_write(self.wfile,{'type':'response.output_text.delta','item_id':'m','output_index':0,'content_index':0,'delta':'ok'})
    ws_write(self.wfile,{'type':'response.completed','response':{**response,'status':'completed','usage':{'input_tokens':500000,'output_tokens':1,'input_tokens_details':{'cached_tokens':499000}}}})
  except (OSError,EOFError):return
 def do_POST(self):
  raw=self.rfile.read(int(self.headers.get('Content-Length','0')));body=json.loads(raw)
  case=self.headers.get('X-Proof-Case','unknown')
  with lock:captured.setdefault(case,[]).append({'body':body,'auth':self.headers.get('Authorization'),'api_key':self.headers.get('x-api-key')})
  kind='anthropic' if self.path.endswith('/messages') else 'chat' if self.path.endswith('/chat/completions') else 'responses'
  usage={'input_tokens':1,'output_tokens':1,'cache_creation_input_tokens':100000}
  if body.get('stream'):
   self.send_response(200);self.send_header('Content-Type','text/event-stream');self.send_header('Connection','close');self.end_headers()
   if kind=='anthropic':first=frame('message_start',{'type':'message_start','message':{'id':'msg-local','type':'message','role':'assistant','model':body['model'],'content':[],'usage':usage}})
   elif kind=='chat':first=b'data: '+json.dumps({'id':'chat-local','object':'chat.completion.chunk','model':body['model'],'choices':[{'index':0,'delta':{'role':'assistant','content':'o'},'finish_reason':None}]}).encode()+b'\n\n'
   else:first=frame('response.created',{'type':'response.created','response':{'id':'resp-local','status':'in_progress','output':[]}})
   self.wfile.write(first);self.wfile.flush();entered.set();release.wait(15)
   if kind=='anthropic':events=[('content_block_start',{'type':'content_block_start','index':0,'content_block':{'type':'text','text':''}}),('content_block_delta',{'type':'content_block_delta','index':0,'delta':{'type':'text_delta','text':'ok'}}),('content_block_stop',{'type':'content_block_stop','index':0}),('message_delta',{'type':'message_delta','delta':{'stop_reason':'end_turn','stop_sequence':None},'usage':{'output_tokens':1}}),('message_stop',{'type':'message_stop'})]
   elif kind=='chat':
    ending={'id':'chat-local','object':'chat.completion.chunk','model':body['model'],'choices':[{'index':0,'delta':{'content':'k'},'finish_reason':'stop'}],'usage':{'prompt_tokens':100001,'completion_tokens':1,'total_tokens':100002,'prompt_tokens_details':{'cached_tokens':100000}}}
    self.wfile.write(b'data: '+json.dumps(ending).encode()+b'\n\ndata: [DONE]\n\n');self.wfile.flush();self.close_connection=True;return
   else:events=[('response.output_text.delta',{'type':'response.output_text.delta','item_id':'m','output_index':0,'content_index':0,'delta':'ok'}),('response.completed',{'type':'response.completed','response':{'id':'resp-local','status':'completed','output':[],'usage':{'input_tokens':500000,'output_tokens':1,'input_tokens_details':{'cached_tokens':499000}}}})]
   for name,event in events:self.wfile.write(frame(name,event));self.wfile.flush()
   self.close_connection=True;return
  if kind=='anthropic':result={'id':'msg-local','type':'message','role':'assistant','model':body['model'],'content':[{'type':'text','text':'ok'}],'stop_reason':'end_turn','stop_sequence':None,'usage':usage}
  elif kind=='chat':result={'id':'chat-local','object':'chat.completion','model':body['model'],'choices':[{'index':0,'message':{'role':'assistant','content':'ok'},'finish_reason':'stop'}],'usage':{'prompt_tokens':100001,'completion_tokens':1,'total_tokens':100002,'prompt_tokens_details':{'cached_tokens':100000}}}
  else:result={'id':'resp-local','object':'response','status':'completed','model':body['model'],'output':[],'usage':{'input_tokens':500000,'output_tokens':1,'input_tokens_details':{'cached_tokens':499000}}}
  data=json.dumps(result).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.flush()
  if case.endswith('-stream'):entered.set();release.wait(15)
  self.wfile.write(data)
fixture_errors=[]
class Server(ThreadingHTTPServer):
 def handle_error(self,request,client_address):fixture_errors.append(type(sys.exception()).__name__)
server=Server(('127.0.0.1',0),Handler);server.daemon_threads=True;thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
port=helper.free_loopback_port();env=helper.safe_environment(run/'state',run/'workspace',run/'config',port)
for p in [run/'workspace',run/'config']:p.mkdir(parents=True,exist_ok=True)
env.update(OPENAI_TARGET_API_URL=f'http://127.0.0.1:{server.server_port}/v1',ANTHROPIC_TARGET_API_URL=f'http://127.0.0.1:{server.server_port}',HEADROOM_CCR_TTL_SECONDS='7200',HEADROOM_TELEMETRY='on',HEADROOM_BEACON='off',HEADROOM_STATELESS='false',HEADROOM_LOG_MESSAGES='off',HEADROOM_LOG_LEVEL='info',HEADROOM_UPDATE_CHECK='off',HEADROOM_MODE='cache',HEADROOM_SAVINGS_PROFILE='coding',HEADROOM_OUTPUT_SHAPER='0',HEADROOM_MODEL_ROUTER_ENABLED='0',HEADROOM_DETECT_BACKEND='python')
env['PATH']=os.pathsep.join((str(runtime),os.path.join(env['SystemRoot'],'System32')))
bootstrap=f"import site,sys;sys.prefix=sys.exec_prefix={str(runtime)!r};site.addsitedir({str(runtime/'Lib/site-packages')!r});sys.path.insert(0,{str(ROOT/'runtime')!r});import cache_control_shim;cache_control_shim.install();import optimization_control;optimization_control.install({generation!r},{secret!r});from headroom.cli import main;main()"
command=[str(python),'-I','-u','-c',bootstrap,'proxy','--host','127.0.0.1','--port',str(port)]
def post(path,body,headers=None,timeout=40,on_headers=None):
 conn=http.client.HTTPConnection('127.0.0.1',port,timeout=timeout)
 try:
  conn.request('POST',path,json.dumps(body).encode(),{'Content-Type':'application/json',**(headers or {})});r=conn.getresponse()
  if on_headers:on_headers(r)
  return r.status,r.read(4*1024*1024)
 finally:conn.close()
revision=0
def control(enabled=None):
 global revision
 body={'schema_version':1,'request_id':str(uuid.uuid4()),'runtime_generation':generation,'action':'read' if enabled is None else 'set'}
 if enabled is not None:body.update(expected_revision=revision,enabled=enabled)
 status,raw=post('/__headroom_desktop/control',body,{'x-headroom-desktop-control':secret});value=json.loads(raw)
 require(status==200 and value['status']=='ok','private_control_failed');revision=value['revision'];return value
def log(which):return '\n'.join(f'2026-01-01T00:00:{i%60:02d}.{i%1000:03d}Z INFO api[worker-{i%8:03d}] request completed status=200 latency_ms=17 region=us-east-1 trace_id={which}{i:012d}' for i in range(200))
def append(body,kind,index):
 body=copy.deepcopy(body);text=log(str(index));call='call-'+str(index)
 if kind=='responses':body['input'] += [{'type':'message','role':'assistant','content':[{'type':'output_text','text':'ok'}]},{'type':'function_call','call_id':call,'name':'inspect_workspace','arguments':'{}'},{'type':'function_call_output','call_id':call,'output':text},{'type':'message','role':'user','content':[{'type':'input_text','text':'summarize the new output'}]}]
 elif kind=='chat':body['messages'] += [{'role':'assistant','content':None,'tool_calls':[{'id':call,'type':'function','function':{'name':'inspect_workspace','arguments':'{}'}}]},{'role':'tool','tool_call_id':call,'content':text}]
 else:body['messages'] += [{'role':'assistant','content':[{'type':'tool_use','id':call,'name':'inspect_workspace','input':{}}]},{'role':'user','content':[{'type':'tool_result','tool_use_id':call,'content':text}]}]
 return body,text
def captured_once(case):
 with lock:rows=copy.deepcopy(captured.get(case,[]))
 require(len(rows)==1,'one_upstream_post_required_'+case+'_'+str(len(rows)));return rows[0]
def without_cache_directives(value):
 if isinstance(value,list):return [without_cache_directives(v) for v in value]
 if isinstance(value,dict):return {k:without_cache_directives(v) for k,v in value.items() if k!='cache_control'}
 return value
def contents(body,kind):
 items=body['input'] if kind=='responses' else body['messages']
 if kind=='responses':return [i.get('output') for i in items if i.get('type')=='function_call_output']
 if kind=='chat':return [i.get('content') for i in items if i.get('role')=='tool']
 return [b.get('content') for i in items for b in i.get('content',[]) if isinstance(b,dict) and b.get('type')=='tool_result']
def tail(body,kind,start):
 body=copy.deepcopy(body)
 if kind=='anthropic':body['messages'] += [{'role':'assistant','content':[{'type':'text','text':'ok'}]},{'role':'user','content':[{'type':'text','text':'inspect the next output'}]}]
 elif kind=='chat':body['messages'] += [{'role':'assistant','content':'ok'},{'role':'user','content':'inspect the next output'}]
 logs=[]
 for i in range(start,start+3):body,text=append(body,kind,i);logs.append(text)
 return body,logs
def run_protocol(kind,explicit=False):
 label=kind+('-explicit' if explicit else '')
 model='claude-sonnet-4-20250514' if kind=='anthropic' else 'o4-mini'
 body={'model':model,'stream':False,**({'max_tokens':2048,'messages':[{'role':'user','content':[{'type':'text','text':'inspect workspace'}]}],'thinking':{'type':'enabled','budget_tokens':1024}} if kind=='anthropic' else {'reasoning':{'effort':'high'},'input':[{'type':'message','role':'user','content':[{'type':'input_text','text':'inspect workspace'}]}]} if kind=='responses' else {'messages':[{'role':'user','content':'inspect workspace'}]})}
 body['tools']=[{'name':'inspect_workspace','description':'Inspect workspace','input_schema':{'type':'object','properties':{}}}] if kind=='anthropic' else [{'type':'function','name':'inspect_workspace','description':'Inspect workspace','parameters':{'type':'object','properties':{}}}] if kind=='responses' else [{'type':'function','function':{'name':'inspect_workspace','description':'Inspect workspace','parameters':{'type':'object','properties':{}}}}]
 # Independent scenarios must not reuse the engine's exact-response cache entry.
 if kind=='anthropic':body['messages'][0]['content'][0]['text']+=' '+label
 path='/v1/messages' if kind=='anthropic' else '/v1/responses' if kind=='responses' else '/v1/chat/completions'
 headers={'Authorization':'Bearer '+SENTINEL,'x-api-key':SENTINEL,'anthropic-version':'2023-06-01','x-headroom-session-id':'pause-'+kind+('-explicit' if explicit else '')}
 control(True)
 first,old=append(body,kind,1)
 for i in range(2,5):first,_=append(first,kind,i)
 if explicit:first['messages'][-1]['content'][0]['cache_control']={'type':'ephemeral','ttl':'1h'}
 case=label+'-first';status,_=post(path,first,{**headers,'X-Proof-Case':case});require(status==200,'first_request_http_'+kind)
 sent_first=captured_once(case);compressed=contents(sent_first['body'],kind)[-1];old=log('4')
 result['compression_diagnostic']={'protocol':kind,'original_bytes':len(old.encode()),'forwarded_tool_bytes':[len(s.encode()) for s in contents(sent_first['body'],kind)]}
 require(isinstance(compressed,str) and len(compressed)<len(old),'first_compression_'+kind)
 hashes=re.findall(r'(?:Retrieve more: hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)',compressed);require(hashes,'ccr_marker_'+kind)
 second,_=tail(first,kind,5);second['stream']=True;case=label+'-stream';results=[];entered.clear();release.clear()
 failures=[];client_sse=threading.Event()
 def stream_call():
  try:results.append(post(path,second,{**headers,'X-Proof-Case':case},on_headers=lambda r:client_sse.set() if 'text/event-stream' in r.getheader('Content-Type','') else None))
  except Exception as exc:failures.append(type(exc).__name__)
 client=threading.Thread(target=stream_call);client.start()
 started=entered.wait(40)
 result['stream_diagnostic']={'protocol':kind,'upstream_started':started,'client_statuses':[r[0] for r in results],'client_error_classes':list(failures),'client_sse_headers':client_sse.wait(3) if started else False}
 require(started,'stream_start_'+kind)
 paused=control(False);require(not paused['enabled'] and paused['active_http_requests']>=1,'pause_ack_active_'+kind)
 release.set();client.join(15);require(not client.is_alive() and results and results[0][0]==200,'stream_complete_'+kind)
 require((b'message_stop' if kind=='anthropic' else b'[DONE]' if kind=='chat' else b'response.completed') in results[0][1],'stream_terminal_event_'+kind)
 sent_second=captured_once(case)
 third,new=tail(second,kind,8);third['stream']=False;case=label+'-paused';status,_=post(path,third,{**headers,'X-Proof-Case':case});require(status==200,'paused_request_http_'+kind)
 sent_third=captured_once(case);key='input' if kind=='responses' else 'messages'
 actual_prefix=sent_third['body'][key][:len(sent_second['body'][key])];expected_prefix=sent_second['body'][key]
 result['prefix_diagnostic']={'protocol':kind,'exact':actual_prefix==expected_prefix,'only_cache_directives_differ':without_cache_directives(actual_prefix)==without_cache_directives(expected_prefix),
  'changed_positions':[i for i,(a,b) in enumerate(zip(actual_prefix,expected_prefix)) if a!=b]}
 require(actual_prefix==expected_prefix,'paused_prefix_'+kind)
 require(contents(sent_third['body'],kind)[-3:]==new,'paused_tail_unchanged_'+kind)
 status,raw=post('/v1/retrieve',{'hash':hashes[0]});require(status==200 and json.loads(raw).get('original_content')==old,'ccr_during_pause_'+kind)
 control(True);fourth,newest=tail(third,kind,11);case=label+'-resumed';status,_=post(path,fourth,{**headers,'X-Proof-Case':case});require(status==200,'resume_request_http_'+kind)
 sent_fourth=captured_once(case)
 resumed_prefix=sent_fourth['body'][key][:len(sent_third['body'][key])]
 exact_resume=resumed_prefix==sent_third['body'][key]
 rendered_resume=without_cache_directives(resumed_prefix)==without_cache_directives(sent_third['body'][key])
 result['resume_prefix_diagnostic']={'protocol':kind,'explicit':explicit,'exact':exact_resume,'rendered_prompt_exact':rendered_resume}
 require(exact_resume if explicit or kind!='anthropic' else rendered_resume,'resumed_prefix_'+kind)
 tail_compacted=any(len(a)<len(b) for a,b in zip(contents(sent_fourth['body'],kind)[-3:],newest))
 _,stats=helper.request_json(port,'/stats?cached=1');logs=stats.get('request_logs',[]);last=logs[-1] if logs else {}
 reason=last.get('tags',{}).get('passthrough_reason')
 allowed_reasons={'bypass_header','compression_disabled','no_messages','license_denied','pre_upstream_backpressure','cache_mode_prefix_mismatch','cache_mode_frozen_cold_start'}
 result['resume_decision_diagnostic']={'protocol':kind,'tail_compacted':tail_compacted,'decision_enabled':reason is None,'reported_disable':reason=='compression_disabled','reason':reason if reason in allowed_reasons else 'unknown' if reason is not None else None,'log_rows':len(logs),'model_matches':last.get('model')==model}
 require(bool(logs) and last.get('model')==model and reason is None,'resume_compression_decision_'+kind)
 require(tail_compacted,'same_conversation_resume_new_compression_'+kind)
 fresh={**copy.deepcopy(body),'stream':False}
 for i in range(100,104):fresh,_=append(fresh,kind,i)
 fresh_headers={**headers,'x-headroom-session-id':'pause-'+label+'-fresh-after-resume','X-Proof-Case':label+'-fresh-after-resume'}
 status,_=post(path,fresh,fresh_headers);require(status==200,'fresh_after_resume_http_'+kind)
 sent_fresh=captured_once(label+'-fresh-after-resume');require(len(contents(sent_fresh['body'],kind)[-1])<len(log('103')),'fresh_compression_after_resume_'+kind)
 for sent in [sent_first,sent_second,sent_third,sent_fourth,sent_fresh]:
  require(sent['body']['model']==model and sent['auth']=='Bearer '+SENTINEL and sent['api_key']==SENTINEL,'model_auth_preserved_'+kind)
  if kind=='responses':require(sent['body']['reasoning']=={'effort':'high'},'reasoning_preserved')
  if kind=='anthropic':require(sent['body']['thinking']==body['thinking'],'thinking_preserved')
 return {'protocol':kind,'explicit_client_marker':explicit,'upstream_posts':5,'active_sse_completed':True,'pause_prefix_exact':True,'paused_tail_utf8_bytes':sum(len(x.encode()) for x in new),'ccr_original_exact':True,'resume_prefix_exact':exact_resume,'resume_rendered_prompt_exact':rendered_resume,'same_conversation_resume_decision_enabled':True,'same_conversation_resumed_tail_compacted':tail_compacted,'fresh_compression_after_resume':True,'model_auth_reasoning_unchanged':True}
def run_direct_anthropic_stream():
 control(True);entered.clear();release.clear();headers_seen=threading.Event();responses=[];failures=[]
 body={'model':'claude-sonnet-4-20250514','max_tokens':128,'stream':True,'messages':[{'role':'user','content':'reply ok'}]}
 headers={'Authorization':'Bearer '+SENTINEL,'x-api-key':SENTINEL,'anthropic-version':'2023-06-01','X-Proof-Case':'anthropic-direct-stream','x-headroom-session-id':'anthropic-direct'}
 def call():
  try:responses.append(post('/v1/messages',body,headers,on_headers=lambda r:headers_seen.set() if 'text/event-stream' in r.getheader('Content-Type','') else None))
  except Exception as e:failures.append(type(e).__name__)
 client=threading.Thread(target=call);client.start()
 try:
  require(entered.wait(40) and headers_seen.wait(3),'direct_sse_emitted_before_pause')
  ack=control(False);require(not ack['enabled'] and ack['active_http_requests']>=1,'direct_active_pause_ack')
 finally:release.set();client.join(15)
 require(not client.is_alive() and not failures and len(responses)==1 and responses[0][0]==200 and b'message_stop' in responses[0][1],'direct_stream_completed_once')
 require(captured_once('anthropic-direct-stream')['body']['stream'] is True,'direct_upstream_stream')
 return {'protocol':'anthropic_direct_sse','headers_emitted_before_pause':True,'terminal_after_pause':True,'upstream_posts':1}
def run_websocket():
 control(True);entered.clear();release.clear();key=base64.b64encode(os.urandom(16)).decode()
 connection=socket.create_connection(('127.0.0.1',port),timeout=45);stream=connection.makefile('rwb')
 headers=f'GET /v1/responses HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {key}\r\nAuthorization: Bearer {SENTINEL}\r\nx-headroom-session-id: pause-ws\r\n\r\n'
 stream.write(headers.encode());stream.flush();require(b'101' in stream.readline(1024),'ws_upgrade')
 while stream.readline(2048) not in (b'\r\n',b''):pass
 body={'type':'response.create','model':'o4-mini','reasoning':{'effort':'high'},'input':[{'type':'message','role':'user','content':[{'type':'input_text','text':'inspect workspace'}]}],'tools':[{'type':'function','name':'inspect_workspace','description':'Inspect workspace','parameters':{'type':'object','properties':{}}}]}
 def send(value):ws_write(stream,value,masked=True)
 def terminal():
  events=[]
  for _ in range(50):
   opcode,data=ws_read(stream)
   if opcode==9:ws_write(stream,data,masked=True,opcode=10);continue
   require(opcode==1,'ws_text_response');event=json.loads(data);events.append(event['type'])
   if event['type']=='response.completed':return events
   require(event['type']!='error','ws_no_protocol_error')
  raise AssertionError('ws_terminal_missing')
 try:
  for i in range(1,5):body,_=append(body,'responses',i)
  send(body);terminal();first=captured_once('responses-ws-1')['body'];compressed=contents(first,'responses')[-1]
  _,stats=helper.request_json(port,'/stats?cached=1');last=stats.get('request_logs',[{}])[-1]
  reason=last.get('tags',{}).get('passthrough_reason');allowed={'compression_disabled','no_messages','license_denied','compression_timeout','cache_mode_prefix_mismatch','cache_mode_frozen_cold_start'}
  result['ws_diagnostic']={'forwarded_tool_lengths':[len(v) for v in contents(first,'responses')],'model_matches':first.get('model')=='o4-mini','passthrough_reason':reason if reason in allowed else 'other' if reason is not None else None}
  require(len(compressed)<len(log('4')),'ws_first_compression');hashes=re.findall(r'(?:Retrieve more: hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)',compressed);require(hashes,'ws_ccr_marker')
  second,_=tail(body,'responses',5);send(second);require(entered.wait(40),'ws_second_active')
  opcode,data=ws_read(stream);require(opcode==1 and json.loads(data)['type']=='response.created','ws_started_event_before_pause')
  require(not control(False)['enabled'],'ws_pause_ack');release.set();terminal();sent_second=captured_once('responses-ws-2')['body']
  third,new=tail(second,'responses',8);send(third);terminal();sent_third=captured_once('responses-ws-3')['body']
  require(sent_third['input'][:len(sent_second['input'])]==sent_second['input'],'ws_paused_prefix_exact');require(contents(sent_third,'responses')[-3:]==new,'ws_paused_tail_raw')
  status,raw=post('/v1/retrieve',{'hash':hashes[0]});require(status==200 and json.loads(raw)['original_content']==log('4'),'ws_paused_ccr')
  control(True);fourth,newest=tail(third,'responses',11);send(fourth);terminal();sent_fourth=captured_once('responses-ws-4')['body']
  require(sent_fourth['input'][:len(sent_third['input'])]==sent_third['input'],'ws_resumed_prefix_exact');require(any(len(a)<len(b) for a,b in zip(contents(sent_fourth,'responses')[-3:],newest)),'ws_resumed_tail_compressed')
  for i in range(1,5):
   row=captured_once('responses-ws-'+str(i));require(row['body']['model']=='o4-mini' and row['body']['reasoning']=={'effort':'high'} and row['auth']=='Bearer '+SENTINEL,'ws_model_auth_reasoning_preserved')
  require(ws_connections[0]==1,'ws_one_upstream_connection')
  ws_write(stream,b'\x03\xe8',masked=True,opcode=8)
  return {'protocol':'responses_websocket','upstream_connections':1,'create_frames':4,'active_response_completed_after_pause':True,'pause_and_resume_prefix_exact':True,'paused_new_tail_raw':True,'ccr_original_exact':True,'same_conversation_resume_new_compression':True,'model_auth_reasoning_unchanged':True}
 finally:release.set();stream.close();connection.close()
process=None;job=helper.KillOnCloseJob();stage='start';result={'passed':False,'run':str(run.relative_to(ROOT)),'paid_model_calls':0,'before':before}
try:
 process=subprocess.Popen(command,cwd=run/'workspace',env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.CREATE_NEW_PROCESS_GROUP)
 job.assign(process.pid)
 output_bytes=[0];engine_diagnostics={}
 def drain():
  for line in iter(lambda:process.stdout.readline(4096),b''):
   output_bytes[0]+=len(line)
   if b'Optimization failed:' in line:
    match=re.search(rb'Optimization failed: ([A-Za-z]+Error|TimeoutError)',line)
    code=match.group(1).decode() if match else 'bounded_optimization_error';engine_diagnostics[code]=engine_diagnostics.get(code,0)+1
 drain_thread=threading.Thread(target=drain,daemon=True);drain_thread.start()
 deadline=time.monotonic()+25
 while time.monotonic()<deadline and process.poll() is None:
  try:
   conn=http.client.HTTPConnection('127.0.0.1',port,timeout=.5);conn.request('GET','/readyz');r=conn.getresponse();ready=r.status==200 and json.loads(r.read()).get('ready') is True;conn.close()
   if ready:break
  except (OSError,http.client.HTTPException,ValueError):pass
  time.sleep(.1)
 else:raise AssertionError('engine_not_ready')
 require(helper.listener_pids(port,ctypes.WinDLL('iphlpapi'))=={'127.0.0.1':process.pid},'owned_listener_required')
 stage='private_control';require(control()['enabled'],'startup_enabled')
 protocols=[]
 selected=sys.argv[1:] or ['responses','chat','anthropic','anthropic_explicit','anthropic_sse','websocket'];require(all(k in ['responses','chat','anthropic','anthropic_explicit','anthropic_sse','websocket'] for k in selected),'invalid_protocol_selection')
 for kind in selected:
  stage=kind;protocols.append(run_websocket() if kind=='websocket' else run_direct_anthropic_stream() if kind=='anthropic_sse' else run_protocol('anthropic',True) if kind=='anthropic_explicit' else run_protocol(kind));result['completed_protocols']=list(protocols)
 result.update(passed=True,protocols=protocols)
except BaseException as exc:
 result.update(error_class=type(exc).__name__,failed_stage=stage,error_code=str(exc) if isinstance(exc,AssertionError) else 'bounded_probe_failed')
finally:
 release.set()
 if process is not None and process.poll() is None:
  process.terminate()
  try:process.wait(5)
  except subprocess.TimeoutExpired:job.close();process.wait(5)
 job.close();server.shutdown();server.server_close()
 result.update(after=fingerprint(),engine_exit_code=process.returncode if process else None,raw_output_saved=False,engine_error_classes=engine_diagnostics,fixture_error_classes=fixture_errors,environment={'platform':sys.platform,'windows_build':list(sys.getwindowsversion()[:3]),'host_python':sys.version.split()[0],'engine_python':'3.13.13'})
 result['inputs_unchanged']=result['before']==result['after'];(run/'result.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf8')
 print(json.dumps({k:v for k,v in result.items() if k not in ['before','after']},ensure_ascii=False))
raise SystemExit(0 if result['passed'] and result['inputs_unchanged'] else 1)
