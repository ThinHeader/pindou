/**
 * 拍照 / 相册选图。
 *
 * 微信从 2023 年起对相册、摄像头走隐私协议校验：没在公众平台声明、或用户没同意过，
 * chooseImage 会直接 fail。原来调用处没写 fail 回调，所以「点了没反应」。
 * 这里统一做三件事：
 *   1. 先申请隐私授权（用户点过一次同意后就不再弹）；
 *   2. 优先用 chooseMedia，基础库不支持时回退 chooseImage；
 *   3. 失败一定有中文原因，取消则安静返回 null。
 */

export type PhotoSource = 'camera' | 'album'

export interface PickedPhoto {
  path: string
  name: string
}

type MediaFile = { tempFilePath?: string; tempFile?: { path?: string } }
type ChooseResult = { tempFilePaths?: string[]; tempFiles?: MediaFile[] }
type ChooseFail = { errMsg?: string; errno?: number; errCode?: number; code?: number }
type WxApi = {
  chooseMedia?: (opts: unknown) => void
  chooseImage?: (opts: unknown) => void
  getPrivacySetting?: (opts: unknown) => void
  requirePrivacyAuthorize?: (opts: unknown) => void
}

function weixin(): WxApi | null {
  // 小程序里 wx 是全局对象；H5 / App 端没有，取不到就走 uni 兜底。
  if (typeof wx !== 'undefined' && wx) return wx as unknown as WxApi
  const holder = globalThis as unknown as { wx?: WxApi }
  return holder.wx && typeof holder.wx === 'object' ? holder.wx : null
}

export async function choosePhoto(source: PhotoSource): Promise<PickedPhoto | null> {
  // 先尝试（不阻塞），失败也继续调，让真正的错误暴露出来。
  await requestPrivacy(weixin()).catch(() => true)
  const attempts = buildAttempts(source)
  if (!attempts.length) throw new Error('当前环境不能选择照片')
  let lastError: unknown = null
  for (const attempt of attempts) {
    try {
      return toPhoto(await attempt())
    } catch (err) {
      if (isCancel(err)) return null
      if (isPrivacy(err)) {
        const agreed = await requestPrivacy(weixin()).catch(() => false)
        if (agreed) {
          try {
            return toPhoto(await attempt())
          } catch (retryErr) {
            if (isCancel(retryErr)) return null
            throw explain(retryErr)
          }
        }
        throw explain(err)
      }
      if (isUnsupported(err)) {
        lastError = err
        continue
      }
      throw explain(err)
    }
  }
  throw explain(lastError)
}

function buildAttempts(source: PhotoSource): Array<() => Promise<ChooseResult>> {
  const list: Array<() => Promise<ChooseResult>> = []
  const api = weixin()
  if (api?.chooseMedia) list.push(() => callApi(api.chooseMedia!, api, 'media', source))
  if (api?.chooseImage) list.push(() => callApi(api.chooseImage!, api, 'image', source))
  if (!api) {
    const uniApi = typeof uni === 'undefined' ? null : (uni as unknown as WxApi)
    if (uniApi && typeof uniApi.chooseMedia === 'function') {
      list.push(() => callApi(uniApi.chooseMedia!, uniApi, 'media', source))
    }
    if (uniApi && typeof uniApi.chooseImage === 'function') {
      list.push(() => callApi(uniApi.chooseImage!, uniApi, 'image', source))
    }
  }
  return list
}

function callApi(
  fn: (opts: unknown) => void,
  receiver: unknown,
  mode: 'media' | 'image',
  source: PhotoSource,
): Promise<ChooseResult> {
  return new Promise((resolve, reject) => {
    const options: Record<string, unknown> = {
      count: 1,
      sizeType: ['compressed'],
      sourceType: [source],
      success: (res: ChooseResult) => resolve(res),
      fail: (err: ChooseFail) => reject(err),
    }
    if (mode === 'media') {
      options.mediaType = ['image']
      options.camera = 'back'
    }
    try {
      fn.call(receiver, options)
    } catch (err) {
      reject(err)
    }
  })
}

function toPhoto(res: ChooseResult | undefined): PickedPhoto {
  const file = res?.tempFiles?.[0]
  const path =
    file?.tempFilePath?.trim() || file?.tempFile?.path?.trim() || res?.tempFilePaths?.[0]?.trim() || ''
  if (!path) throw new Error('没有取到照片，请重试')
  return { path, name: nameOf(path) }
}

function nameOf(path: string): string {
  const clean = path.split('?')[0].split('#')[0]
  const last = clean.split('/').pop() || ''
  try {
    return decodeURIComponent(last) || '参考图'
  } catch {
    return last || '参考图'
  }
}

function requestPrivacy(api: WxApi | null): Promise<boolean> {
  if (!api?.getPrivacySetting) return Promise.resolve(true)
  return new Promise((resolve) => {
    let settled = false
    const done = (value: boolean) => {
      if (settled) return
      settled = true
      resolve(value)
    }
    api.getPrivacySetting!({
      success: (res: { needAuthorization?: boolean }) => {
        if (!res?.needAuthorization || !api.requirePrivacyAuthorize) {
          done(true)
          return
        }
        api.requirePrivacyAuthorize!({
          success: () => done(true),
          fail: () => done(false),
        })
      },
      fail: () => done(true),
    })
  })
}

function errMsgOf(err: unknown): string {
  const msg = (err as ChooseFail | undefined)?.errMsg
  return typeof msg === 'string' ? msg : err instanceof Error ? err.message : ''
}

function isCancel(err: unknown): boolean {
  return /cancel/i.test(errMsgOf(err))
}

function isPrivacy(err: unknown): boolean {
  const fail = err as ChooseFail | undefined
  const code = fail?.errno ?? fail?.errCode ?? fail?.code
  if (code === 112) return true
  return /privacy/i.test(errMsgOf(err))
}

function isUnsupported(err: unknown): boolean {
  return /not support|unsupported|is not a function|no such method|not exist/i.test(errMsgOf(err))
}

function explain(err: unknown): Error {
  if (err instanceof Error && !isPrivacy(err)) return err
  const detail = errMsgOf(err)
  if (isPrivacy(err)) {
    return new Error(
      '微信还没允许使用相册/相机：需要在公众平台「设置 - 服务内容声明 - 用户隐私保护指引」里声明「选中的照片或视频信息」和「摄像头」，审核通过后重新进入小程序再试',
    )
  }
  if (/auth den|authorize|permission/i.test(detail)) {
    return new Error('没有相册或相机权限，请在微信里允许后重试')
  }
  return new Error(detail ? `打开相册失败：${detail.slice(0, 40)}` : '打开相册失败，请重试')
}
