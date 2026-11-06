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
assert.strictEqual(show('OPENAI_API_KEY=sk-proj-4bG7kLm2Qx9Rt5Vw8Yz1Np3s'), 'OPENAI_API_KEY=sk-proj-••••••••••••••••••••Np3s');

// Passwords in URLs and auth headers.
assert.strictEqual(show('postgres://app:p4ssw0rd@db.internal:5432/main'), 'postgres://app:••••••••@db.internal:5432/main');
assert.strictEqual(show('app:p4ssw0rd@db.internal'), 'app:••••••••@db.internal');
assert.strictEqual(show('Authorization: Bearer abcdef0123456789'), 'Authorization: Bearer ••••••••••••••••');
assert.strictEqual(show('redis://:p4ss@cache:6379'), 'redis://:••••@cache:6379');
assert.strictEqual(show('Authorization: Basic dXNlcjpodW50ZXIy'), 'Authorization: Basic ' + b(16));

// Whole values, whatever they hold: spaces when quoted, & and ; when they
// aren't the start of the next name.
assert.strictEqual(show('Password: Tr0ub4dor&3'), 'Password: ' + b(11));
assert.strictEqual(show('password: "correct horse battery staple"'), 'password: "' + b(28) + '"');
assert.strictEqual(show('Server=db;User Id=sa;Password=S3cr3t!;Encrypt=true'), 'Server=db;User Id=sa;Password=•••••••;Encrypt=true');
assert.strictEqual(show("'password' => 'hunter22',"), "'password' => '••••••••',");
assert.strictEqual(show('password := "hunter22"'), 'password := "••••••••"');
assert.strictEqual(show('Passwort: Sommer2024!'), 'Passwort: ' + b(11));
assert.strictEqual(show('PIN: 4711'), 'PIN: ••••');
assert.strictEqual(show('DB_PASS=hunter22'), 'DB_PASS=••••••••');
assert.strictEqual(show('X-Api-Key: 9f8e7d6c5b4a'), 'X-Api-Key: ' + b(12));
assert.strictEqual(show('bot 123456789:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw0 here'), 'bot 123456789:' + b(31) + 'saw0 here');

// Ordinary text is left alone.
for (const s of ['Meeting moved to Thursday 14:00', 'billing@acme-industries.com', 'https://github.com/anthropics',
  'const total = items.reduce((sum, i) => sum + i.price, 0);', 'call +49 30 1234567', '#18181b']) {
  assert.deepStrictEqual(S.find(s), [], s);
}

// …and so is everything that only mentions a secret: settings that aren't
// one, types, flags, placeholders and where the secret is read from.
for (const s of ['max_tokens: 4096', 'token_type: bearer', 'secret_name: prod-db', 'passwordless: true', 'bypass: true', 'spin=3',
  "credentials: 'include',", 'password: string;', 'password=${DB_PASS}', 'password: process.env.DB_PASSWORD,', 'token: this.token,',
  'const pw = getPassword();', 'password: ********', 'password: <your password>', 'PASSWORD=\nUSER=bob', 'postgres://user:${PASSWORD}@db/x',
  'Basic understanding of things', 'Bearer authentication is common', 'xai-grok-vision-beta-model']) {
  assert.deepStrictEqual(S.find(s), [], s);
}
// Numbers pass as cards only when a card network issues them.
assert.deepStrictEqual(S.find('2000 0000 0000 0008'), []);
assert.strictEqual(show('2223 0000 4840 0011'), '•••••••••••••••0011');

// Each secret says what it is, once, in the order it appears.
const types = (s) => S.types(s).map((t) => `${t.label}/${t.icon}`);
assert.deepStrictEqual(types('DE89370400440532013000'), ['IBAN/bank']);
assert.deepStrictEqual(types('4111 1111 1111 1111'), ['Visa card/card']);
assert.deepStrictEqual(types('5555-5555-5555-4444'), ['Mastercard/card']);
assert.deepStrictEqual(types('3782 822463 10005'), ['Amex card/card']);
assert.deepStrictEqual(types('ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ012345'), ['GitHub token/key']);
assert.deepStrictEqual(types('sk_' + 'live_51NzQ8vKd2Lr7XbW4eTfYp9Hm'), ['Stripe key/key']);
assert.deepStrictEqual(types('AKIAIOSFODNN7EXAMPLE'), ['AWS key/key']);
assert.deepStrictEqual(types('eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl'), ['JWT/key']);
assert.deepStrictEqual(types('DB_PASSWORD=hunter22'), ['Password/lock']);
assert.deepStrictEqual(types('postgres://app:p4ssw0rd@db:5432/main'), ['Database URL/db']);
assert.deepStrictEqual(types('https://bob:p4ssw0rd@example.com'), ['URL password/lock']);
assert.deepStrictEqual(types('Authorization: Bearer abcdef0123456789'), ['Bearer token/key']);
assert.deepStrictEqual(types('-----BEGIN OPENSSH PRIVATE KEY-----\nb3Bl\n-----END OPENSSH PRIVATE KEY-----'), ['SSH key/key']);
assert.deepStrictEqual(types('STRIPE_SECRET_KEY=sk_' + 'live_51NzQ8vKd2Lr7XbW4eTfYp9Hm\nDATABASE_URL=postgres://app:Tr0ub4dor@db/main\nREDIS_PASSWORD=x9y8z7'),
  ['Stripe key/key', 'Database URL/db', 'Password/lock']);
assert.deepStrictEqual(types('PASSWORD=a1b2c3\nOTHER_PASSWORD=d4e5f6'), ['Password/lock']);
assert.deepStrictEqual(types('Meeting moved to Thursday 14:00'), []);

// A private key is hidden from just past its BEGIN line, PGP blocks included.
assert.strictEqual(S.privateKey('-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAA\n-----END OPENSSH PRIVATE KEY-----'), 35);
assert.strictEqual(S.privateKey('key:\n-----BEGIN PGP PRIVATE KEY BLOCK-----\nlQOYBF'), 42);
assert.strictEqual(S.privateKey('-----BEGIN PUBLIC KEY-----'), -1);

console.log('secrets ok');
