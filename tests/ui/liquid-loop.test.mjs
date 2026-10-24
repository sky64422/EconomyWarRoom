import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createLiquidLoop } from '../../src/ui/liquid-loop.ts';
import { createNotchSurface } from '../../src/ui/notch-surface.ts';

const flush = async()=>{for(let i=0;i<12;i++)await Promise.resolve();};
test('capture is serialized and an old theme frame is discarded before release',async()=>{
 let tick,resolve;const calls=[];
 const loop=createLiquidLoop({request:cb=>{tick=cb;return 1;},cancel:()=>{},interval:()=>0,
 capture:()=>{calls.push('capture');return new Promise(r=>resolve=r);},
 paint:v=>calls.push('paint:'+v),dispose:v=>calls.push('dispose:'+v),release:async()=>{calls.push('release');},fail:e=>{throw e;}});
 loop.setEnabled(true);tick(0);tick(16);assert.deepEqual(calls,['capture']);
 loop.setEnabled(false);loop.setEnabled(true);resolve('old');await flush();
 assert.deepEqual(calls,['capture','dispose:old','release']);
 tick(32);assert.equal(calls.at(-1),'capture');loop.destroy();resolve('last');await flush();
 assert.equal(calls.at(-1),'release');assert(!calls.some(x=>x.startsWith('paint')));
});
test('hidden capture starts no requests, failed capture releases and can retry',async()=>{
 let tick;let captures=0,releases=0,failures=0;
 const loop=createLiquidLoop({request:cb=>{tick=cb;return 1;},cancel:()=>{},interval:()=>0,
 capture:async()=>{captures++;throw Error('unavailable');},paint:()=>{},dispose:()=>{},release:async()=>{releases++;},fail:()=>{failures++;}});
 assert.equal(captures,0);loop.setEnabled(true);tick(0);await flush();assert.equal(failures,1);assert.equal(releases,1);
 loop.setEnabled(false);await flush();loop.setEnabled(true);tick(32);await flush();assert.equal(failures,2);loop.destroy();
});
test('surface serializes geometry and suppresses stale painted layouts',async()=>{
 const pending=[],painted=[];
 const surface=createNotchSurface({send:r=>new Promise(resolve=>pending.push({r,resolve})),paint:l=>painted.push(l),fail:e=>{throw e;}});
 surface.request({expanded:true,height:100,target:80,providers:['a']});await flush();
 surface.request({expanded:true,height:400,target:150,providers:['b']});
 pending[0].resolve({name:'old'});await flush();assert.equal(painted.length,0);
 pending[1].resolve({name:'new'});await surface.settled();assert.deepEqual(painted,[{name:'new'}]);surface.destroy();
});
test('idle loop with interval Infinity pauses animation frames and wakes up on refresh', async () => {
  let requests = 0, tick = null;
  let currentInterval = Infinity;
  const loop = createLiquidLoop({
    request: cb => { requests++; tick = cb; return requests; },
    cancel: () => {},
    interval: () => currentInterval,
    capture: async () => 'frame',
    paint: () => {},
    dispose: () => {},
    release: async () => {},
    fail: () => {},
  });
  loop.setEnabled(true);
  assert.equal(requests, 1);
  tick(0);
  await flush();
  // While interval is Infinity, schedule() must not queue further frames
  assert.equal(requests, 1);

  // Calling refresh() must wake up and request next frame
  loop.refresh();
  assert.equal(requests, 2);
  loop.destroy();
});
