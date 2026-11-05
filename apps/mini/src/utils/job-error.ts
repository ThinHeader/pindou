/**
 * 出稿失败的稳定错误码 → 用户文案。
 * `content_policy` 的句子必须和 API `CONTENT_POLICY_MESSAGE` 保持一致。
 * 未列出的失败继续用服务端原文（通用「请稍后重试」）。
 */
export const CONTENT_POLICY_CODE = 'content_policy'

export const CONTENT_POLICY_MESSAGE =
  '这张图受版权保护或内容受限，请换一张图或换个主体再试。本次未扣积分。'

export const JOB_ERROR_COPY: Record<string, string> = {
  [CONTENT_POLICY_CODE]: CONTENT_POLICY_MESSAGE,
}

export const CONTENT_POLICY_NOTE =
  '同一张图再试也会被拦住。换一张图，或换一个主体更清楚的照片。本次未扣积分。'

const GENERIC_FAILURE_NOTE = '可稍后再试，或换一张主体更清楚的照片。失败不计费，积分仍保留。'

export interface JobFailureFields {
  error_code?: string | null
  error?: string | null
  message?: string | null
}

/** 出稿失败展示：已登记的错误码用专用文案，否则用服务端 `error` / `message`。 */
export function jobFailureCopy(
  job: JobFailureFields | null | undefined,
  fallback = '出稿失败。本次未扣积分。',
): string {
  const code = job?.error_code?.trim()
  if (code) {
    const mapped = JOB_ERROR_COPY[code]
    if (mapped) return mapped
  }
  const fromServer = (job?.error || job?.message || '').trim()
  return fromServer || fallback
}

export function isContentPolicy(errorCode: string | null | undefined): boolean {
  return errorCode === CONTENT_POLICY_CODE
}

export function jobFailureNote(errorCode: string | null | undefined): string {
  return isContentPolicy(errorCode) ? CONTENT_POLICY_NOTE : GENERIC_FAILURE_NOTE
}
