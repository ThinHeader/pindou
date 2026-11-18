import assert from 'node:assert/strict'
import { test } from 'node:test'
import { choosePhoto } from '../src/utils/photo.ts'

type Options = {
  success?: (res: unknown) => void
  fail?: (err: unknown) => void
  [key: string]: unknown
}

function installWx(api: Record<string, (opts: Options) => void>) {
  ;(globalThis as { wx?: unknown }).wx = api
}

function removeWx() {
  delete (globalThis as { wx?: unknown }).wx
}

test('chooseMedia 成功后返回临时文件路径', async () => {
  installWx({
    chooseMedia: (opts) => opts.success?.({ tempFiles: [{ tempFilePath: 'wxfile://tmp/a.jpg' }] }),
  })
  const photo = await choosePhoto('album')
  assert.deepEqual(photo, { path: 'wxfile://tmp/a.jpg', name: 'a.jpg' })
  removeWx()
})

test('用户取消时安静返回 null', async () => {
  installWx({
    chooseMedia: (opts) => opts.fail?.({ errMsg: 'chooseMedia:fail cancel' }),
    chooseImage: (opts) => opts.fail?.({ errMsg: 'chooseImage:fail cancel' }),
  })
  assert.equal(await choosePhoto('camera'), null)
  removeWx()
})

test('基础库没有 chooseMedia 时回退 chooseImage', async () => {
  installWx({
    chooseImage: (opts) => opts.success?.({ tempFilePaths: ['wxfile://tmp/b.png'] }),
  })
  const photo = await choosePhoto('camera')
  assert.equal(photo?.path, 'wxfile://tmp/b.png')
  removeWx()
})

test('隐私接口未声明时给出可操作的提示', async () => {
  let authorized = 0
  installWx({
    getPrivacySetting: (opts) => opts.success?.({ needAuthorization: true }),
    requirePrivacyAuthorize: (opts) => {
      authorized += 1
      opts.fail?.({ errMsg: 'requirePrivacyAuthorize:fail' })
    },
    chooseMedia: (opts) =>
      opts.fail?.({
        errMsg: 'chooseMedia:fail api scope is not declared in the privacy agreement',
        errno: 112,
      }),
  })
  await assert.rejects(
    () => choosePhoto('album'),
    (err: Error) => /隐私保护指引/.test(err.message) && /摄像头/.test(err.message),
  )
  assert.ok(authorized >= 1)
  removeWx()
})

test('未授权过隐私协议时先弹授权再重试一次', async () => {
  let picks = 0
  installWx({
    getPrivacySetting: (opts) => opts.success?.({ needAuthorization: false }),
    requirePrivacyAuthorize: (opts) => opts.success?.(undefined),
    chooseMedia: (opts) => {
      picks += 1
      if (picks === 1) {
        opts.fail?.({ errMsg: 'chooseMedia:fail privacy permission is not authorized', errno: 104 })
        return
      }
      opts.success?.({ tempFiles: [{ tempFilePath: 'wxfile://tmp/c.jpg' }] })
    },
  })
  const photo = await choosePhoto('album')
  assert.equal(photo?.path, 'wxfile://tmp/c.jpg')
  assert.equal(picks, 2)
  removeWx()
})
