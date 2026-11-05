import test from 'node:test';import assert from 'node:assert/strict';
import {decodeAttributionSummary} from '../../src/lib/metrics.ts';
const hash=char=>char.repeat(64);
const totals=()=>({status:'measured',before_tokens:100,after_tokens:50,saved_tokens:50,saved_percent:50,comparable_events:1,excluded_events:0});
function row(char,role='main',upstream='c'){
 return {group_id:hash(char),model:'same-model',provider:'provider',client:'codex',upstream_id:hash(upstream),agent:{id:hash(char),parent_id:null,role},conflicted:false,is_other:false,
  coverage:{requests:1,events:1,model_observed:1,client_verified:1,agent_verified:role==='unknown'?0:1,conflicts:0},totals:totals()};
}
function fixture(groups=[row('a')]){
 const coverage={requests:0,events:0,model_observed:0,client_verified:0,agent_verified:0,conflicts:0};
 for(const group of groups)for(const field of Object.keys(coverage))coverage[field]+=group.coverage[field];
 const status=count=>count===0?'unknown':count===coverage.requests?'verified':'partial';
 return {scope:'observed_requests',coverage,client_status:status(coverage.client_verified),agent_status:status(coverage.agent_verified),groups,omitted_groups:0};
}
function main(groups){const eligible=groups.filter(row=>row.totals.status==='measured');const measured=eligible.length>0;
 return {status:measured?'measured':'no_comparable_events',comparable_events:groups.reduce((n,row)=>n+row.totals.comparable_events,0),excluded_events:groups.reduce((n,row)=>n+row.totals.excluded_events,0),
 before_tokens:measured?eligible.reduce((n,row)=>n+row.totals.before_tokens,0):null,after_tokens:measured?eligible.reduce((n,row)=>n+row.totals.after_tokens,0):null,
 saved_tokens:measured?eligible.reduce((n,row)=>n+row.totals.saved_tokens,0):null,saved_percent:measured?50:null};}
const decode=value=>decodeAttributionSummary(value,value.coverage.events,value.coverage.requests,main(value.groups));
test('same model never merges distinct agent and upstream rows; only allowlisted metadata leaves decoder',()=>{
 const value=fixture([row('a'),row('b','subagent','d')]);value.groups[0].raw_messages='PRIVATE_CANARY';
 const result=decode(value);assert.equal(result.groups.length,2);assert.notEqual(result.groups[0].agent.id,result.groups[1].agent.id);
 assert.notEqual(result.groups[0].upstream_id,result.groups[1].upstream_id);assert.equal(result.client_status,'verified');
 assert.equal(JSON.stringify(result).includes('CANARY'),false);
});
test('unknown metadata stays unknown; absent role keeps partial observed scope',()=>{
 const unknown=row('b');Object.assign(unknown,{model:null,client:null,upstream_id:null,agent:null,coverage:{requests:1,events:1,model_observed:0,client_verified:0,agent_verified:0,conflicts:0}});
 const value=fixture([row('a'),unknown,row('c','unknown')]);const result=decode(value);
 assert.equal(result.agent_status,'partial');assert.equal(result.client_status,'partial');assert.equal(result.groups[1].agent,null);
 assert.equal(result.scope,'observed_requests');
 const empty=fixture([]);assert.deepEqual(decode(empty),empty);
});
test('forged certainty, duplicate identities, wrong counts and unsafe labels are rejected',()=>{
 const mutations=[v=>v.agent_status='verified-all-agents',v=>v.client_status='unknown',v=>v.scope='all_requests',
 v=>v.coverage.requests=2,v=>v.coverage.events=100001,v=>v.groups.push(structuredClone(v.groups[0])),v=>v.groups[0].client=null,
 v=>v.groups[0].agent.parent_id=hash('a'),v=>v.groups[0].agent.role='leader',v=>v.groups[0].agent.id='raw-id',
 v=>v.groups[0].model='https://PRIVATE_CANARY',v=>v.groups[0].provider='bad\nlabel',v=>v.groups[0].model='x'.repeat(129),v=>v.groups[0].model='sk-PRIVATE_CANARY',
 v=>v.groups[0].totals.saved_tokens=99,v=>v.groups[0].coverage.client_verified=0,v=>v.groups[0].coverage.model_observed=0,
 v=>v.groups[0].conflicted=true,v=>v.groups[0].is_other=true,v=>v.groups[0].upstream_id=null];
 for(const mutate of mutations){const value=fixture();mutate(value);assert.equal(decode(value),null,mutate.toString());}
 assert.equal(decodeAttributionSummary(fixture(),2,1,totals()),null);
});
test('legacy unknown token basis is retained alongside observed model labels',()=>{
 const value=fixture();value.groups[0].totals={status:'no_comparable_events',before_tokens:null,after_tokens:null,saved_tokens:null,saved_percent:null,comparable_events:0,excluded_events:1};
 const result=decode(value);assert.equal(result.groups[0].model,'same-model');assert.equal(result.groups[0].totals.saved_percent,null);
});
test('other row preserves counts and totals without inventing identities',()=>{
 const groups=Array.from({length:64},(_,i)=>{const value=row('a');value.group_id=i.toString(16).padStart(64,'0');return value;});
 const last=groups[63];Object.assign(last,{model:null,provider:null,client:null,upstream_id:null,agent:null,is_other:true});
 last.coverage={requests:2,events:2,model_observed:2,client_verified:2,agent_verified:2,conflicts:0};last.totals={...totals(),before_tokens:200,after_tokens:100,saved_tokens:100,comparable_events:2};
 const value=fixture(groups);value.omitted_groups=2;assert.equal(decode(value).groups.length,64);
 value.omitted_groups=3;assert.equal(decode(value),null);
});
