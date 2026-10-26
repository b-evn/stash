const assert = require('assert');
const S = require('./search.js');

const mk = (text, o = {}) => ({ id: Math.random(), text, len: text.length, kind: 'text', pinned: false, uses: 0, last_used: 1000, ...o });
const texts = (r) => r.map((x) => x.clip.text);

const clips = [
  mk('GitHub pull request #42', { last_used: 5 }),
  mk('https://github.com/anthropics', { kind: 'link', last_used: 4, uses: 9 }),
  mk('grocery list: milk, eggs', { last_used: 3, pinned: true }),
  mk('const gh = require("gh")', { kind: 'code', last_used: 2 }),
  mk('hello world', { last_used: 1 }),
];

assert.deepStrictEqual(texts(S.run(clips, 'ghb')).slice(0, 1), ['GitHub pull request #42']);
assert.strictEqual(S.run(clips, 'zzz').length, 0);
assert.deepStrictEqual(texts(S.run(clips, 'pull 42')), ['GitHub pull request #42']);
const hit = S.run(clips, 'wor')[0];
assert.deepStrictEqual([...hit.idx], [6, 7, 8]);
assert.strictEqual(texts(S.run(clips, 'git'))[0], 'GitHub pull request #42');
assert.ok(!texts(S.run(clips, 'gh')).includes('hello world'));
assert.ok(!texts(S.run([mk('meeting moved to thursday 14:00')], 'gh')).length);
assert.ok(!S.run([mk('Meeting moved to Thursday')], 'inv').length);
assert.ok(S.run([mk('git pull --rebase')], 'gpr').length);
assert.deepStrictEqual([...S.hits('inv', 'invoice-0923.pdf')], [0, 1, 2]);
assert.deepStrictEqual(texts(S.run(clips, "'wrld")), []);
assert.deepStrictEqual(texts(S.run(clips, "'world")), ['hello world']);
assert.deepStrictEqual(texts(S.run(clips, '^hello')), ['hello world']);
assert.ok(!texts(S.run(clips, 'git !pull')).includes('GitHub pull request #42'));
assert.ok(S.run(clips, 'GH').length > 0);
assert.ok(S.run(clips, 'GH').every((r) => /G.*H/.test(r.clip.text)));
assert.deepStrictEqual(texts(S.run(clips, '@link')), ['https://github.com/anthropics']);
assert.deepStrictEqual(texts(S.run(clips, '@pin')), ['grocery list: milk, eggs']);
assert.deepStrictEqual(texts(S.run(clips, '@code gh')), ['const gh = require("gh")']);
assert.deepStrictEqual(texts(S.run(clips, '', { filter: 'code' })), ['const gh = require("gh")']);
assert.deepStrictEqual(texts(S.run(clips, '', { pinned: true })), ['grocery list: milk, eggs']);
const withFiles = clips.concat([mk('C:\\docs\\invoice.pdf', { kind: 'file' }), mk('', { kind: 'image' })]);
assert.deepStrictEqual(texts(S.run(withFiles, 'invoice @file')), ['C:\\docs\\invoice.pdf']);
assert.strictEqual(S.run(withFiles, '', { filter: 'image' }).length, 1);
assert.strictEqual(S.run(withFiles, '', { filter: 'text' }).length, 3);
assert.strictEqual(S.run(clips, '@hour', { now: 1000 + 36e5 + 1 }).length, 0);
assert.strictEqual(texts(S.run(clips, ''))[0], 'grocery list: milk, eggs');
assert.strictEqual(texts(S.run(clips, '', { sort: 'frequent' }))[1], 'https://github.com/anthropics');
assert.deepStrictEqual(texts(S.run(clips, '', { sort: 'a–z' })).slice(1, 3), ['const gh = require("gh")', 'GitHub pull request #42']);
assert.strictEqual(S.parse('@nonsense').terms.length, 1);
assert.ok(S.run([mk('', { kind: 'image', ocr_text: 'invoice total $4,200' })], 'invoice').length);
assert.ok(S.run([mk('', { kind: 'image', ocr_text: 'Q4 goals ship v2 API' })], 'q4').length);
assert.strictEqual(S.run([mk('', { kind: 'image', ocr_text: 'no match here' })], 'xyzzy').length, 0);
assert.deepStrictEqual([...S.hits('total', 'invoice total $4,200')], [8, 9, 10, 11, 12]);
console.log('search ok');
