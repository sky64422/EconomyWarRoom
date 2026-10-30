import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyPanelOpacity, opacityToPct, pctToOpacity } from '../../src/ui/opacity.ts';

test('Liquid preserves endpoints and the chosen 50 and 70 percent appearances', () => {
  const properties = {};
  const element = { style: { setProperty: (name, value) => { properties[name] = value; } } };
  globalThis.document = { documentElement: element };
  let previous = -1;
  for (const [percent, opacity, strength] of [[0, 0.35, 0], [50, 0.675, .15], [70, .805, .25], [100, 1, 1]]) {
    assert.equal(pctToOpacity(percent), opacity);
    assert.equal(opacityToPct(opacity), percent);
    applyPanelOpacity(element, opacity);
    const actual = Number(properties['--liquid-opacity']);
    assert.ok(Math.abs(actual - strength) < 0.000001);
    assert.ok(actual > previous);
    previous = actual;
  }
  delete globalThis.document;
});
