import { test } from 'node:test';
import assert from 'node:assert/strict';
import { sparklinePaths } from '../../src/ui/sparkline.ts';

test('sparkline endpoint stays at the last observed session time', () => {
  const points=[{t:0,close:100},{t:50,close:110}];
  const paths=sparklinePaths(points,220,52,4,100,{start:0,end:100});
  assert.equal(paths.lastPoint.x,110);
  assert.ok(paths.lastPoint.y>=4 && paths.lastPoint.y<=48);
  assert.deepEqual(points,[{t:0,close:100},{t:50,close:110}]);
});

test('empty and single-point sparklines do not fabricate a trend', () => {
  assert.equal(sparklinePaths([],220,52).lastPoint,null);
  const one=sparklinePaths([{t:0,close:100}],220,52,4,100,{start:0,end:100});
  assert.equal(one.lastPoint.x,4);
  assert.equal(one.line.includes('L'),false);
  assert.ok(Number.isFinite(one.lastPoint.y));
});
