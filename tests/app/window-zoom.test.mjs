import test from 'node:test';
import assert from 'node:assert/strict';
import {createWindowZoomClient} from '../../src/lib/windowZoom.ts';
test('window scale sends only the three supported native factors',async()=>{
 const calls=[];const client=createWindowZoomClient({isNative:()=>true,setZoom:async scale=>{calls.push(scale);}});
 for(const scale of [1,1.25,1.5])assert.equal(await client.set(scale),true);
 assert.deepEqual(calls,[1,1.25,1.5]);
});
test('unavailable desktop or unsupported scales do not call native zoom',async()=>{
 let calls=0;const client=createWindowZoomClient({isNative:()=>false,setZoom:async()=>{calls++;}});
 assert.equal(await client.set(1.25),false);
 const native=createWindowZoomClient({isNative:()=>true,setZoom:async()=>{calls++;}});
 for(const scale of [0,Infinity,NaN,2,'1.25'])assert.equal(await native.set(scale),false);
 assert.equal(calls,0);
});
test('native failure returns a fixed failure without surfacing exception content',async()=>{
 const client=createWindowZoomClient({isNative:()=>true,setZoom:async()=>{throw Error('PRIVATE_CANARY');}});
 assert.equal(await client.set(1.5),false);
});
