import assert from 'node:assert/strict'
import { beforeEach, test } from 'node:test'
import {
  bindSessionRefresh,
  notifySessionExpired,
  refreshSessionSingleFlight,
  resetSessionRefreshForTests,
} from '../src/api/session.ts'
import { TOKEN_REFRESH_AHEAD_MS, jwtExpMillis, tokenNeedsRefresh } from '../src/utils/jwt.ts'

const SAMPLE =
  'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyIiwiZXhwIjoxNzAwMDAwMDAwfQ.sig'

function tokenWithExp(exp: number): string {
  const header = Buffer.from(JSON.stringify({ alg: 'HS256', typ: 'JWT' })).toString('base64url')
  const payload = Buffer.from(JSON.stringify({ sub: 'user', exp })).toString('base64url')
  return `${header}.${payload}.sig`
}

beforeEach(() => {
  resetSessionRefreshForTests()
})

test('reads exp from a jwt payload without a library', () => {
  assert.equal(jwtExpMillis(SAMPLE), 1_700_000_000_000)
})

test('refreshes when exp is inside two days, missing, or already past', () => {
  const now = 1_700_000_000_000
  assert.equal(tokenNeedsRefresh(tokenWithExp((now + TOKEN_REFRESH_AHEAD_MS) / 1000), now), false)
  assert.equal(
    tokenNeedsRefresh(tokenWithExp((now + TOKEN_REFRESH_AHEAD_MS - 1000) / 1000), now),
    true,
  )
  assert.equal(tokenNeedsRefresh(tokenWithExp(now / 1000 - 10), now), true)
  assert.equal(tokenNeedsRefresh('not-a-jwt', now), true)
  assert.equal(tokenNeedsRefresh('a.!!!.c', now), true)
  assert.equal(jwtExpMillis(tokenWithExp(now / 1000)), now)
})

test('concurrent refresh shares one login', async () => {
  let calls = 0
  let release: () => void = () => {}
  const gate = new Promise<void>((resolve) => {
    release = resolve
  })
  bindSessionRefresh(async () => {
    calls += 1
    await gate
    return true
  }, () => {})
  const first = refreshSessionSingleFlight()
  const second = refreshSessionSingleFlight()
  assert.equal(calls, 1)
  release()
  assert.deepEqual(await Promise.all([first, second]), [true, true])
  assert.equal(await refreshSessionSingleFlight(), true)
  assert.equal(calls, 2)
})

test('a thrown refresh fails once for every waiter', async () => {
  let calls = 0
  bindSessionRefresh(async () => {
    calls += 1
    throw new Error('wx login failed')
  }, () => {})
  const [a, b] = await Promise.all([refreshSessionSingleFlight(), refreshSessionSingleFlight()])
  assert.equal(a, false)
  assert.equal(b, false)
  assert.equal(calls, 1)
})

test('expired session notifies once per burst', () => {
  let n = 0
  bindSessionRefresh(async () => false, () => {
    n += 1
  })
  notifySessionExpired()
  notifySessionExpired()
  assert.equal(n, 1)
})
