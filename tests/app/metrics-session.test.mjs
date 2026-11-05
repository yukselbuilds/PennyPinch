import test from 'node:test';
import assert from 'node:assert/strict';
import { createOverviewSession } from '../../src/lib/metricsSession.ts';

const turn = () => new Promise(resolve => setImmediate(resolve));
function harness() {
  const calls = [], states = [];
  const session = createOverviewSession(period => new Promise(resolve => calls.push({period, resolve})), s => states.push(s));
  return { session, calls, states, latest: () => states.at(-1) };
}
const result = period => ({kind:'snapshot', snapshot:{data:{period}, marker:period}});

test('late reply never appears under a newer period; rapid choices coalesce without overlapping reads', async () => {
  const h=harness(); h.session.target('session',true);
  h.session.target('today',true); h.session.target('seven_days',true);
  assert.equal(h.calls.length,1); assert.deepEqual(h.latest(),{period:'seven_days',metrics:{kind:'loading'}});
  h.calls[0].resolve(result('session')); await turn();
  assert.equal(h.calls.length,2); assert.equal(h.calls[1].period,'seven_days');
  assert.equal(h.latest().metrics.kind,'loading');
  h.calls[1].resolve(result('seven_days')); await turn();
  assert.equal(h.latest().period,'seven_days'); assert.equal(h.latest().metrics.snapshot.marker,'seven_days');
});
test('same-period failure keeps the last snapshot visibly stale, refresh recovers', async () => {
  const h=harness(); h.session.target('today',true);
  h.calls[0].resolve(result('today')); await turn();
  h.session.refresh(); assert.equal(h.latest().metrics.refreshing,true);
  h.calls[1].resolve({kind:'metrics_error',code:'busy'}); await turn();
  assert.equal(h.latest().metrics.stale,true); assert.equal(h.latest().metrics.issue,'busy');
  assert.equal(h.latest().metrics.snapshot.marker,'today');
  h.session.refresh(); h.calls[2].resolve(result('today')); await turn();
  assert.equal(h.latest().metrics.stale,false);
});
test('inactive view ignores replies and refreshes, resumes with a new read', async () => {
  const h=harness(); h.session.target('session',true); h.session.target('session',false);
  h.calls[0].resolve(result('session')); await turn(); h.session.refresh();
  assert.equal(h.calls.length,1); assert.equal(h.states.length,0);
  h.session.target('session',true); assert.equal(h.calls.length,2);
  h.calls[1].resolve({kind:'desktop_unavailable'}); await turn();
  assert.equal(h.latest().metrics.kind,'unavailable');
});
test('wrong scope and rejected transport produce fixed errors, never an old scope', async () => {
  const h=harness(); h.session.target('today',true); h.calls[0].resolve(result('session')); await turn();
  assert.deepEqual(h.latest().metrics,{kind:'error',error:'invalid_response'});
  h.session.refresh(); h.calls[1].resolve({kind:'request_failed'}); await turn();
  assert.deepEqual(h.latest().metrics,{kind:'error',error:'request_failed'});
});
