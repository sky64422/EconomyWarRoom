import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyPanelOpacity, opacityToPct, pctToOpacity } from '../../src/ui/opacity.ts';

test('opacity slider maps its full range to the Liquid material', () => {
  const properties = {};
  const element = { style: { setProperty: (name, value) => { properties[name] = value; } } };
  globalThis.document = { documentElement: element };
  for (const [percent, opacity] of [[0, 0.35], [50, 0.675], [100, 1]]) {
    assert.equal(pctToOpacity(percent), opacity);
    assert.equal(opacityToPct(opacity), percent);
    applyPanelOpacity(element, opacity);
    assert.ok(Math.abs(Number(properties['--liquid-opacity']) - percent / 100) < 0.000001);
  }
  delete globalThis.document;
});
