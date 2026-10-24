import { test } from 'node:test';
import assert from 'node:assert/strict';
import { initialNotchState, reduceNotchState, selectCells, primaryPercent } from '../../src/ui/notch-state.ts';
import { localSuggestions } from '../../src/ui/watchlist/search.ts';

test('localSuggestions matches Korean domestic equities and Korean aliases of global symbols', () => {
  const kr = localSuggestions('삼성', []);
  assert.ok(kr.some((s) => s.symbol === '005930.KS' && s.name === '삼성전자'));
  const us = localSuggestions('테슬라', []);
  assert.ok(us.some((s) => s.symbol === 'TSLA'));
  const crypto = localSuggestions('비트코인', []);
  assert.ok(crypto.some((s) => s.symbol === 'BTC-USD'));
});

test('notch uses latest primary percent, preserves zero and missing', () => {
  assert.equal(primaryPercent({ display: { primary: { change: 1.24 }, secondary: { change: -0.36 } } }), 1.24);
  assert.equal(primaryPercent({ display: { primary: { change: 0 } } }), 0);
  assert.equal(primaryPercent({ regular_change_percent: 5, change_percent: 8 }), null);
  assert.equal(primaryPercent({ display: { primary: { change: NaN } } }), null);
});
test('all slots show instruments unless overflow needs a list button', () => {
  const ids = Array.from({length: 10}, (_, i) => String(i));
  assert.deepEqual(selectCells(ids, 6), { ids: ['0','1','2','3','4'], overflow: 5, empty: false });
  assert.deepEqual(selectCells(ids, 1), { ids: [], overflow: 10, empty: false });
  assert.deepEqual(selectCells(['a'], 6), { ids: ['a'], overflow: 0, empty: false });
  assert.deepEqual(selectCells(['a'], 1), { ids: ['a'], overflow: 0, empty: false });
  assert.deepEqual(selectCells(ids.slice(0,6), 6), { ids: ids.slice(0,6), overflow: 0, empty: false });
  assert.deepEqual(selectCells(ids, 2), { ids: ['0'], overflow: 9, empty: false });
  assert.deepEqual(selectCells([], 6), { ids: [], overflow: 0, empty: true });
});
test('pin, hover, settings and removal preserve selection intent', () => {
  let s = reduceNotchState(initialNotchState(), { type: 'pin', id: 'a' });
  assert.equal(s.itemId, 'a'); assert.equal(s.pinned, true);
  assert.deepEqual(reduceNotchState(s, { type: 'hover', id: 'b' }), s);
  s = reduceNotchState(s, {type:'pin', id:'b'});
  assert.equal(s.itemId, 'b'); assert.equal(s.pinned, true);
  assert.equal(reduceNotchState(s, {type:'pin', id:'b'}).itemId, null);
  s = reduceNotchState(s, {type:'settings'});
  s = reduceNotchState(s, {type:'settings'});
  assert.equal(s.settings, false); assert.equal(s.itemId, 'b');
  assert.equal(reduceNotchState(s, {type:'items', ids:['a']}).itemId, null);
});

test('update event toggles update view and clears settings and item selection', () => {
  let s = initialNotchState();
  s = reduceNotchState(s, { type: 'update' });
  assert.equal(s.update, true);
  assert.equal(s.settings, false);
  assert.equal(s.itemId, null);
  s = reduceNotchState(s, { type: 'update' });
  assert.equal(s.update, false);

  s = reduceNotchState(s, { type: 'update' });
  assert.equal(s.update, true);
  s = reduceNotchState(s, { type: 'pin', id: 'a' });
  assert.equal(s.update, false);
  assert.equal(s.itemId, 'a');

  s = reduceNotchState(s, { type: 'update' });
  assert.equal(s.update, true);
  s = reduceNotchState(s, { type: 'settings' });
  assert.equal(s.update, false);
  assert.equal(s.settings, true);
});
