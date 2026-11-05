import test from 'node:test';import assert from 'node:assert/strict';
import {createRuntimeClient,decodeSnapshot} from '../../src/runtime/snapshot.ts';
import {optimizationView} from '../../src/lib/optimization.ts';
import {runtimeSnapshot,runtimeRequestId,runtimeGeneration} from '../fixtures/runtime.mjs';
function client(response=runtimeSnapshot('ready','paused',1)){
 const calls=[];return {calls,api:createRuntimeClient({isNative:()=>true,createRequestId:()=>runtimeRequestId,invoke:async(command,args)=>{calls.push({command,args});return typeof response==='function'?response():response;}})};
}
test('known optimization requires a ready owned engine and safe revision',()=>{
 for(const state of ['enabled','paused'])assert.equal(decodeSnapshot(runtimeSnapshot('ready',state,0),runtimeRequestId)?.compression.status,state);
 for(const mutate of [r=>r.compression.revision=null,r=>r.compression.revision=-1,r=>r.compression.revision=Number.MAX_SAFE_INTEGER+1,r=>r.compression.active_stream_count=1,
  r=>r.engine.status='stopping',r=>r.compression.status='unknown',r=>r.runtime_generation='0'.repeat(32)]){
  const value=runtimeSnapshot();mutate(value);assert.equal(decodeSnapshot(value,runtimeRequestId),null);
 }
 const unknown=runtimeSnapshot('ready','unknown',null);unknown.capabilities.pause=false;assert.ok(decodeSnapshot(unknown,runtimeRequestId));
 const limited=runtimeSnapshot();limited.capabilities.pause=false;assert.ok(decodeSnapshot(limited,runtimeRequestId));
 const unavailable=runtimeSnapshot('stopped');unavailable.capabilities.pause=true;assert.equal(decodeSnapshot(unavailable,runtimeRequestId),null);
});
test('control IPC sends only expected generation, revision and desired state once',async()=>{
 const h=client();const result=await h.api.set(runtimeGeneration,0,false);assert.equal(result.kind,'snapshot');
 assert.deepEqual(h.calls,[{command:'set_optimization',args:{requestId:runtimeRequestId,expectedGeneration:runtimeGeneration,expectedRevision:0,enabled:false}}]);
 assert.equal(JSON.stringify(result).includes('control_token'),false);
});
test('wrong ack or transport failure never becomes paused and never retries',async()=>{
 for(const reply of [runtimeSnapshot(),runtimeSnapshot('ready','paused',3),{...runtimeSnapshot('ready','paused',1),runtime_generation:'a'.repeat(32)},
  {...runtimeSnapshot('ready','paused',1),control_token:'PRIVATE_CANARY'}]){
  const h=client(reply);assert.deepEqual(await h.api.set(runtimeGeneration,0,false),{kind:'invalid_response'});assert.equal(h.calls.length,1);
 }
 const failed=client(()=>{throw Error('PRIVATE_CANARY');});assert.deepEqual(await failed.api.set(runtimeGeneration,0,false),{kind:'request_failed'});assert.equal(failed.calls.length,1);
 const h=client();assert.deepEqual(await h.api.set('../PRIVATE_CANARY',0,false),{kind:'invalid_response'});assert.deepEqual(await h.api.set(runtimeGeneration,NaN,false),{kind:'invalid_response'});assert.equal(h.calls.length,0);
});
test('stale generation or revision returns fixed correlated error only',async()=>{
 const error={schema_version:1,request_id:runtimeRequestId,status:'error',error:{code:'config_conflict',retryable:false,message_key:'runtime.command.config_conflict',diagnostic_id:null}};
 const h=client(error);assert.equal((await h.api.set(runtimeGeneration,0,false)).kind,'snapshot_error');assert.equal(h.calls.length,1);
});
test('primary action follows confirmed source and all pending phases lock it',()=>{
 assert.equal(optimizationView(runtimeSnapshot('stopped'),null).primary,'start');
 assert.equal(optimizationView(runtimeSnapshot(),null).primary,'pause');assert.equal(optimizationView(runtimeSnapshot('ready','paused',1),null).primary,'resume');
 const unknown=runtimeSnapshot('ready','unknown',null);unknown.capabilities.pause=false;
 assert.equal(optimizationView(unknown,null).primary,'refresh');assert.equal(optimizationView(unknown,null).phase,'unknown');
 for(const action of ['start','pause','resume','stop'])assert.equal(optimizationView(runtimeSnapshot(),action).disabled,true);
 const limited=runtimeSnapshot();limited.capabilities.pause=false;const view=optimizationView(limited,null);assert.equal(view.disabled,true);assert.equal(view.pauseUnavailable,true);
 assert.equal(optimizationView(null,null).phase,'unknown');
});
