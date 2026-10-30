import assert from 'node:assert/strict';
import test from 'node:test';
import { getBinaryName } from './util.mjs';

for (const [platform, arch, expected] of [
  ['linux', 'x64', 'decay-x64'],
  ['linux', 'arm64', 'decay-arm64'],
  ['darwin', 'arm64', 'decay-darwin-arm64'],
]) {
  test(`${platform}/${arch} selects ${expected}`, () => {
    assert.equal(getBinaryName(platform, arch), expected);
  });
}
for (const [platform, arch] of [['darwin', 'x64'], ['linux', 'ia32'], ['win32', 'x64']]) {
  test(`${platform}/${arch} is rejected`, () => {
    assert.throws(() => getBinaryName(platform, arch), /does not support/);
  });
}
