const assert = require('assert');
const S = require('./secrets.js');

// Shows s with its blurred ranges as •, to compare against what stays visible.
const b = (n) => '•'.repeat(n);
const show = (s) => {
  const out = [...s];
  for (const [a, b] of S.find(s)) for (let i = a; i < b; i++) out[i] = '•';
  return out.join('');
};

// IBANs keep country, check digits and the last four; invalid ones are left alone.
assert.strictEqual(show('DE89370400440532013000'), 'DE89' + b(14) + '3000');
assert.strictEqual(show('IBAN: DE89 3704 0044 0532 0130 00'), 'IBAN: DE89 ' + b(17) + '30 00');
assert.strictEqual(show('GB29 NWBK 6016 1331 9268 19'), 'GB29 ' + b(17) + '68 19');
assert.strictEqual(show('DE00370400440532013000'), 'DE00370400440532013000');

// Card numbers keep the last four, and only when the Luhn check passes.
assert.strictEqual(show('4111 1111 1111 1111'), '•••••••••••••••1111');
assert.strictEqual(show('card 5555-5555-5555-4444 exp 12/29'), 'card •••••••••••••••4444 exp 12/29');
assert.strictEqual(show('4111 1111 1111 1112'), '4111 1111 1111 1112');
// Millisecond timestamps start with 1, so they never look like a card.
assert.strictEqual(show('1793023865319'), '1793023865319');

// API tokens keep their prefix and last four.
assert.strictEqual(show('sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123'), 'sk-ant-api03-••••••••••••••••••••••••••0123');
assert.strictEqual(show('ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ012345'), 'ghp_••••••••••••••••••••••••••••2345');
assert.strictEqual(show('key AKIAIOSFODNN7EXAMPLE.'), 'key AKIA••••••••••••MPLE.');
assert.strictEqual(show('eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl'), 'eyJ' + b(42) + 'dXJl');
assert.strictEqual(show('ask-me-anything'), 'ask-me-anything');
assert.strictEqual(show('task-abcdefghijklmnopqrstuvwxyz'), 'task-abcdefghijklmnopqrstuvwxyz');

// Assigned secrets are blurred whole; the name stays.
assert.strictEqual(show('DB_PASSWORD=hunter22'), 'DB_PASSWORD=••••••••');
assert.strictEqual(show('"client_secret": "s3cr3t-value",'), '"client_secret": "••••••••••••",');
assert.strictEqual(show('example.com/cb?access_token=abc123&state=x'), 'example.com/cb?access_token=••••••&state=x');
// …except a token with a known prefix, which keeps its prefix.
assert.strictEqual(show('OPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwx'), 'OPENAI_API_KEY=sk-proj-••••••••••••••••••••uvwx');

// Passwords in URLs and auth headers.
assert.strictEqual(show('postgres://app:p4ssw0rd@db.internal:5432/main'), 'postgres://app:••••••••@db.internal:5432/main');
assert.strictEqual(show('app:p4ssw0rd@db.internal'), 'app:••••••••@db.internal');
assert.strictEqual(show('Authorization: Bearer abcdef0123456789'), 'Authorization: Bearer ••••••••••••••••');

// Ordinary text is left alone.
for (const s of ['Meeting moved to Thursday 14:00', 'billing@acme-industries.com', 'https://github.com/anthropics',
  'const total = items.reduce((sum, i) => sum + i.price, 0);', 'call +49 30 1234567', '#18181b']) {
  assert.deepStrictEqual(S.find(s), [], s);
}

assert.ok(S.privateKey('-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAA\n-----END OPENSSH PRIVATE KEY-----'));
assert.ok(!S.privateKey('-----BEGIN PUBLIC KEY-----'));

console.log('secrets ok');
