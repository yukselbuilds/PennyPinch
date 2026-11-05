import test from 'node:test';import assert from 'node:assert/strict';
import {createActivityClient,decodeActivityFilter} from '../../src/lib/activityClient.ts';
import {defaultActivityFilter} from '../../src/lib/activity.ts';
import {activityList,activityDetail,activityRequestId,hash} from '../fixtures/activity.mjs';
const filter=defaultActivityFilter;
function harness(reply=activityList()){
 const calls=[];const client=createActivityClient({isNative:()=>true,createRequestId:()=>activityRequestId,invoke:async(command,args)=>{calls.push({command,args});return typeof reply==='function'?reply(command,args):reply;}});
 return {client,calls};
}
test('activity list and detail preserve zero, unknown, negative and layer scope',async()=>{
 const h=harness((command,args)=>command==='get_activity_snapshot'?activityList(args.request.period,args.request.filter):activityDetail(args.request.period,args.request.origin_id));
 const list=await h.client.read('today',filter);assert.equal(list.kind,'snapshot');assert.equal(list.snapshot.data.rows[0].metadata.total_ms,0);
 assert.equal(list.snapshot.data.rows[1].totals.saved_percent,-25);assert.equal(list.snapshot.data.rows[2].totals.saved_percent,null);
 const detail=await h.client.detail('today',hash('a'));assert.equal(detail.kind,'snapshot');assert.equal(detail.snapshot.data.row.totals.saved_tokens,50);assert.equal(detail.snapshot.data.measurements[1].saved_tokens,90);
 assert.deepEqual(h.calls[1],{command:'get_activity_detail',args:{request:{schema_version:1,request_id:activityRequestId,period:'today',origin_id:hash('a')}}});
});
test('allowlist drops secret content at every exported level',async()=>{
 const canary='PRIVATE_CONTENT_CANARY';
 for(const mode of ['list','detail']){
  const raw=mode==='list'?activityList():activityDetail();
  raw.error_body=canary;raw.data.request_messages=[canary];
  const row=mode==='list'?raw.data.rows[0]:raw.data.row;row.secret=canary;row.identity.auth=canary;row.metadata.error=canary;row.totals.raw=canary;
  if(mode==='detail'){raw.data.measurements[0].error=canary;raw.data.measurements[0].before.tokenizer=canary;raw.data.measurements[0].provider_usage.auth=canary;}
  const h=harness(raw);const result=mode==='list'?await h.client.read('today',filter):await h.client.detail('today',hash('a'));
  assert.equal(result.kind,'snapshot');assert.equal(JSON.stringify(result).includes(canary),false);
 }
});
test('bad correlation, bounds, model, metadata and scope are fixed invalid responses',async()=>{
 const mutate=[r=>r.request_id=crypto.randomUUID(),r=>r.data.period='session',r=>r.data.limit=101,r=>r.data.has_more=true,
  r=>r.data.rows.push(r.data.rows[0]),r=>r.data.rows[0].identity.model='C:/PRIVATE_CANARY',r=>r.data.rows[0].identity.provider='/Users/PRIVATE_CANARY',r=>r.data.rows[0].metadata.total_ms=-1,
  r=>r.data.rows[0].metadata.total_ms=Infinity,r=>r.data.rows[0].metadata.transforms_count=129,r=>r.data.rows[0].metadata.conflicted=true,
  r=>r.data.rows[0].timestamp_utc='2026-11-06T12:00:00Z',r=>r.data.rows.reverse(),r=>r.data.rows[0].totals.saved_percent=51];
 for(const change of mutate){const raw=activityList();change(raw);assert.deepEqual(await harness(raw).client.read('today',filter),{kind:'error',error:'invalid_response'});}
 const raw=activityList();raw.data.rows[0].identity.model='unknown';assert.equal((await harness(raw).client.read('today',filter)).kind,'error');
});
test('filters, opaque origins and desktop availability bound native calls',async()=>{
 const h=harness();assert.deepEqual(await h.client.read('today',{...filter,model:{kind:'exact',value:'sk-PRIVATE_CANARY'}}),{kind:'error',error:'invalid_input'});
 assert.deepEqual(await h.client.detail('today','../PRIVATE_CANARY'),{kind:'error',error:'invalid_input'});assert.equal(h.calls.length,0);
 const f={...filter,client:'codex',model:{kind:'exact',value:'test-model-a'}};
 assert.deepEqual(decodeActivityFilter({...f,secret:'PRIVATE_CANARY'}),f);
 const list=activityList('today',f);assert.equal((await harness(list).client.read('today',f)).kind,'snapshot');
 assert.equal((await harness(activityList()).client.read('today',f)).kind,'error');
 const c=createActivityClient({isNative:()=>false,createRequestId:()=>null,invoke:async()=>{throw Error('PRIVATE_CANARY');}});
 assert.deepEqual(await c.read('today',filter),{kind:'unavailable'});
});
test('detail rejects mismatched origin, duplicate measurement, false percent and raw basis',async()=>{
 const mutate=[r=>r.data.row.origin_id=hash('b'),r=>r.data.measurements[1].event_id=r.data.measurements[0].event_id,
  r=>r.data.measurements[0].saved_percent=99,r=>r.data.measurements[0].before.basis_id='PRIVATE_CANARY',r=>r.data.omitted_measurements=1,
  r=>r.data.measurements[1].main_metric_eligible=true];
 for(const change of mutate){const raw=activityDetail();change(raw);assert.deepEqual(await harness(raw).client.detail('today',hash('a')),{kind:'error',error:'invalid_response'});}
 const error={schema_version:1,request_id:activityRequestId,status:'error',code:'not_found',retryable:false,body:'PRIVATE_CANARY'};
 assert.deepEqual(await harness(error).client.detail('today',hash('a')),{kind:'error',error:'not_found'});
 const throwing=harness(()=>{throw Error('PRIVATE_CANARY');});assert.deepEqual(await throwing.client.detail('today',hash('a')),{kind:'error',error:'request_failed'});
});
