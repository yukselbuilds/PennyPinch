import test from 'node:test';import assert from 'node:assert/strict';
import {createActivitySession} from '../../src/lib/activitySession.ts';
import {defaultActivityFilter} from '../../src/lib/activity.ts';
import {activityList,activityDetail,hash} from '../fixtures/activity.mjs';
const turn=()=>new Promise(resolve=>setImmediate(resolve));
function harness(){const calls=[],states=[],details=[];
 const session=createActivitySession({read:(period,filter)=>new Promise(resolve=>calls.push({period,filter,resolve})),detail:(period,id)=>new Promise(resolve=>details.push({period,id,resolve}))},v=>states.push(v));
 return {session,calls,details,states,latest:()=>states.at(-1)};
}
const ok=snapshot=>({kind:'snapshot',snapshot});
test('new filter and period coalesce while late list results are discarded',async()=>{
 const h=harness();h.session.target('today',defaultActivityFilter,true);h.session.target('seven_days',{...defaultActivityFilter,client:'unknown'},true);
 assert.equal(h.calls.length,1);h.calls[0].resolve(ok(activityList()));await turn();assert.equal(h.calls.length,2);assert.equal(h.latest().state.kind,'loading');
 h.calls[1].resolve(ok(activityList('seven_days',h.calls[1].filter)));await turn();assert.equal(h.latest().state.snapshot.data.rows[0].origin_id,hash('c'));
});
test('detail selection and filter changes discard old detail responses',async()=>{
 const h=harness();h.session.target('today',defaultActivityFilter,true);h.calls[0].resolve(ok(activityList()));await turn();
 h.session.select(hash('a'));h.session.select(hash('b'));assert.equal(h.details.length,1);
 h.details[0].resolve(ok(activityDetail()));await turn();assert.equal(h.latest().detail.kind,'loading');assert.equal(h.details.length,2);
 h.details[1].resolve(ok(activityDetail('today',hash('b'))));await turn();assert.equal(h.latest().detail.snapshot.data.row.origin_id,hash('b'));
 h.session.refresh();h.session.target('today',{...defaultActivityFilter,client:'unknown'},true);assert.equal(h.latest().selectedId,null);
 h.details[2].resolve(ok(activityDetail('today',hash('b'))));h.calls[1].resolve(ok(activityList()));await turn();assert.equal(h.latest().detail.kind,'loading');
});
test('same scope failures visibly preserve history, inactive views ignore replies',async()=>{
 const h=harness();h.session.target('today',defaultActivityFilter,true);h.calls[0].resolve(ok(activityList()));await turn();
 h.session.refresh();h.calls[1].resolve({kind:'error',error:'busy'});await turn();assert.equal(h.latest().state.stale,true);assert.equal(h.latest().state.issue,'busy');
 h.session.refresh();h.session.target('today',defaultActivityFilter,false);h.calls[2].resolve(ok(activityList()));await turn();
 assert.equal(h.latest().state.kind,'loading');h.session.refresh();assert.equal(h.calls.length,3);
 h.session.target('today',defaultActivityFilter,true);h.calls[3].resolve({kind:'unavailable'});await turn();assert.equal(h.latest().state.kind,'unavailable');
});
