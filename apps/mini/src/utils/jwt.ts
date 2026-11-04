/** 剩余不足这个时间就静默重登。服务端 JWT 仍是 7 天。 */
export const TOKEN_REFRESH_AHEAD_MS = 2 * 24 * 60 * 60 * 1000

const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'

function decodeBase64Url(segment: string): string | null {
  if (!segment || segment.length % 4 === 1 || /[^A-Za-z0-9_-]/.test(segment)) return null
  const normalized = segment.replace(/-/g, '+').replace(/_/g, '/')
  const padded = normalized + '='.repeat((4 - (normalized.length % 4)) % 4)
  const bytes: number[] = []
  for (let i = 0; i < padded.length; i += 4) {
    const c2 = padded[i + 2]
    const c3 = padded[i + 3]
    const a = B64.indexOf(padded[i])
    const b = B64.indexOf(padded[i + 1])
    const c = c2 === '=' ? -1 : B64.indexOf(c2)
    const d = c3 === '=' ? -1 : B64.indexOf(c3)
    if (a < 0 || b < 0 || (c2 !== '=' && c < 0) || (c3 !== '=' && d < 0) || (c < 0 && d >= 0)) {
      return null
    }
    bytes.push((a << 2) | (b >> 4))
    if (c >= 0) bytes.push(((b & 0x0f) << 4) | (c >> 2))
    if (d >= 0 && c >= 0) bytes.push(((c & 0x03) << 6) | d)
  }
  let text = ''
  for (let i = 0; i < bytes.length; i += 1) text += String.fromCharCode(bytes[i])
  return text
}

/** 只读 payload 的 exp（毫秒）。不校验签名，鉴权仍由服务端完成。 */
export function jwtExpMillis(token: string): number | null {
  const segment = token.split('.')[1]
  if (!segment) return null
  try {
    const json = decodeBase64Url(segment)
    if (!json) return null
    const payload: unknown = JSON.parse(json)
    if (!payload || typeof payload !== 'object' || Array.isArray(payload)) return null
    const exp = (payload as { exp?: unknown }).exp
    if (typeof exp !== 'number' || !Number.isFinite(exp)) return null
    return exp * 1000
  } catch {
    return null
  }
}

export function tokenNeedsRefresh(token: string, now = Date.now()): boolean {
  const exp = jwtExpMillis(token)
  if (exp == null) return true
  return exp - now < TOKEN_REFRESH_AHEAD_MS
}
