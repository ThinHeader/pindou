import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  CONTENT_POLICY_CODE,
  CONTENT_POLICY_MESSAGE,
  JOB_ERROR_COPY,
  isContentPolicy,
  jobFailureCopy,
  jobFailureNote,
} from '../src/utils/job-error.ts'

const BANNED = ['AI', 'Seedream', 'seedream', '模型', '深度合成', '豆包', '方舟', '大模型']

test('content_policy maps to the dedicated plain-language copy', () => {
  assert.equal(JOB_ERROR_COPY[CONTENT_POLICY_CODE], CONTENT_POLICY_MESSAGE)
  const copy = jobFailureCopy({
    error_code: CONTENT_POLICY_CODE,
    error: '出稿失败，请稍后重试。本次未扣积分。',
    message: '出稿失败，请稍后重试。本次未扣积分。',
  })
  assert.equal(copy, CONTENT_POLICY_MESSAGE)
  assert.match(copy, /换一张图/)
  assert.match(copy, /换个主体/)
  assert.match(copy, /未扣积分/)
  for (const word of BANNED) assert.equal(copy.includes(word), false, word)
  assert.equal(copy.toLowerCase().includes('copyright'), false)
  assert.equal(copy.includes('PolicyViolation'), false)
})

test('other failures keep the server copy', () => {
  const generic = '出稿失败，请稍后重试。本次未扣积分。'
  assert.equal(jobFailureCopy({ error_code: null, error: generic, message: '正在整理色块' }), generic)
  assert.equal(jobFailureCopy({ error: '', message: '出稿超时了。本次未扣积分。' }), '出稿超时了。本次未扣积分。')
  assert.equal(
    jobFailureCopy({ error_code: 'unknown_code', error: generic }),
    generic,
  )
  assert.equal(jobFailureCopy(null), '出稿失败。本次未扣积分。')
  assert.equal(isContentPolicy(null), false)
  assert.equal(isContentPolicy(''), false)
  assert.equal(isContentPolicy(CONTENT_POLICY_CODE), true)
})

test('content policy note asks for another image and other failures stay generic', () => {
  const policy = jobFailureNote(CONTENT_POLICY_CODE)
  const generic = jobFailureNote(null)
  assert.match(policy, /换一张图/)
  assert.match(policy, /未扣积分/)
  assert.equal(generic, '可稍后再试，或换一张主体更清楚的照片。失败不计费，积分仍保留。')
  assert.notEqual(policy, generic)
  for (const word of BANNED) assert.equal(policy.includes(word), false, word)
})
