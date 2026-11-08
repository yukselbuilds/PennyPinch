import test from 'node:test';import assert from 'node:assert/strict';
import {createCompatibilityClient,decodeCompatibilitySnapshot} from '../../src/lib/compatibility.ts';
import {compatibilitySnapshot} from '../fixtures/compatibility.mjs';
import {decodeDiscoveryReply} from '../../src/lib/integrations.ts';
import {invoke as fixtureInvoke,reset as resetFixture} from '../fixtures/desktopTransport.ts';
const id='7132bced-3ce0-4c1b-8f48-d93d7b96f7ce';
test('desktop discovery fixture passes the real decoder before compatibility UI scenarios',async()=>{
 for(const scenario of ['data','empty','error']){
  resetFixture('en','dark',scenario);
  for(const command of ['get_discovery_snapshot','scan_discovery']){
   const raw=await fixtureInvoke(command,{requestId:id});assert.equal(decodeDiscoveryReply(raw,id).kind,'snapshot');
  }
 }
});
function client(invoke,extra={}){return createCompatibilityClient({isAvailable:()=>true,requestId:()=>id,invoke,...extra});}

test('compatibility keeps historical auth scope and development update policy separate',async()=>{
 let calls=0;const result=await client(async(command,args)=>{calls++;assert.equal(command,'get_compatibility_snapshot');assert.deepEqual(args,{request:{schema_version:1,request_id:id}});return compatibilitySnapshot(id);}).getSnapshot();
 assert.equal(result.kind,'snapshot');assert.equal(calls,1);assert.equal(result.snapshot.updater_available,false);
 const record=result.snapshot.records.find(r=>r.id==='codex-current-real-api-key-provider');assert.equal(record.evidence_status,'not_tested');
 assert.equal(result.snapshot.records.find(r=>r.id==='claude-code-native-product-scoped-fixture').evidence_status,'local_fixture_pass');
});
test('compatibility rejects forged scope, stale matches, unsafe evidence and invalid contracts',()=>{
 const changes=[v=>{v.request_id='wrong';},v=>{v.raw_error='private';},v=>{v.automatic_updates=true;},v=>{v.engine.active_version='0.42.0';},
  v=>{v.records[0].evidence=['../../secret'];},v=>{v.records[0].evidence_date='2026-11-31';},v=>{v.records[1].id=v.records[0].id;},
  v=>{v.records.find(r=>r.evidence_status==='live_verified').auth_method='synthetic';},v=>{v.installations[0].freshness='stale';},
  v=>{v.installations[0].version='0.167.0';},v=>{v.installations[0].client_kind='claude_code';},v=>{v.scan_generation=null;},
  v=>{v.adapters[1].client_kind='codex';},v=>{v.installations[0].historical_evidence_ids.push('missing');},v=>{v.installations[0].config_supported=false;v.installations[0].config_state='matches_owned_endpoint';},v=>{v.records[0].capability_counts[0].count=0;},v=>{v.records[0].capability_counts[0].status='all_verified';}];
 for(const change of changes){const value=compatibilitySnapshot(id);change(value);assert.equal(decodeCompatibilitySnapshot(value,id),null);}
});
test('compatibility handles desktop absence, IPC errors and public errors without leaking raw content',async()=>{
 let calls=0;assert.equal((await client(async()=>{calls++;},{isAvailable:()=>false}).getSnapshot()).kind,'desktop_unavailable');assert.equal(calls,0);
 assert.equal((await client(async()=>{throw Error('private');}).getSnapshot()).kind,'request_failed');
 for(const code of ['invalid_input','busy','unavailable']){
  const result=await client(async()=>({schema_version:1,request_id:id,status:'error',error:{code,retryable:code==='busy',message_key:`compatibility.${code}`,diagnostic_id:null}})).getSnapshot();assert.deepEqual(result,{kind:'catalog_error',code});
 }
});
test('compatibility releases single-flight after a failed request',async()=>{
 let reject;const first=new Promise((_,r)=>{reject=r;});let calls=0;const c=client(async()=>++calls===1?first:compatibilitySnapshot(id));
 const pending=c.getSnapshot();assert.equal((await c.getSnapshot()).kind,'client_busy');reject(Error('private'));
 assert.equal((await pending).kind,'request_failed');assert.equal((await c.getSnapshot()).kind,'snapshot');assert.equal(calls,2);
});
