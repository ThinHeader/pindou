/** 把登录头像收成现有 POST /api/v1/me/profile 能解析的 multipart。不改接口。 */

const BOUNDARY = '----pindouProfileBoundary'

export function encodeProfileMultipart(
  nickname: string,
  file: Uint8Array,
): { body: ArrayBuffer; contentType: string } {
  const name = nickname.replace(/[\r\n]/g, '').trim()
  const text = new TextEncoder()
  const chunks: Uint8Array[] = []
  if (name) {
    chunks.push(
      text.encode(
        `--${BOUNDARY}\r\nContent-Disposition: form-data; name="nickname"\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n${name}\r\n`,
      ),
    )
  }
  chunks.push(
    text.encode(
      `--${BOUNDARY}\r\nContent-Disposition: form-data; name="file"; filename="avatar.jpg"\r\nContent-Type: application/octet-stream\r\n\r\n`,
    ),
  )
  chunks.push(file)
  chunks.push(text.encode(`\r\n--${BOUNDARY}--\r\n`))
  const size = chunks.reduce((total, chunk) => total + chunk.byteLength, 0)
  const out = new Uint8Array(size)
  let offset = 0
  for (const chunk of chunks) {
    out.set(chunk, offset)
    offset += chunk.byteLength
  }
  return {
    body: out.buffer,
    contentType: `multipart/form-data; boundary=${BOUNDARY}`,
  }
}

/**
 * chooseAvatar 在开发者工具是 http://tmp/...，真机是 wxfile://tmp_...。
 * 直接交给 uploadFile 会 fail。拷到用户目录后变成 wxfile://usr/...，uploadFile 才能读。
 * 拷不了就退回原路径（H5 的 blob 地址走这条）。
 */
export function copyAvatarForUpload(src: string): Promise<string> {
  const filePath = src.trim()
  if (!filePath) return Promise.reject({ message: '头像读取失败' })
  const fs = typeof uni.getFileSystemManager === 'function' ? uni.getFileSystemManager() : null
  const dir = userDataDir()
  if (!fs || !dir) return Promise.resolve(filePath)
  const dest = `${dir}/pindou-avatar.jpg`
  return new Promise((resolve) => {
    const write = () => {
      fs.readFile({
        filePath,
        success: (res) => {
          const bytes = toBytes(res.data)
          if (!bytes?.byteLength) {
            resolve(filePath)
            return
          }
          const data = new Uint8Array(bytes.byteLength)
          data.set(bytes)
          fs.writeFile({
            filePath: dest,
            data: data.buffer,
            success: () => resolve(dest),
            fail: () => resolve(filePath),
          })
        },
        fail: () => resolve(filePath),
      })
    }
    if (typeof fs.unlink === 'function') fs.unlink({ filePath: dest, complete: () => write() })
    else write()
  })
}

/** chooseAvatar / chooseImage 的临时文件。读得到字节即可，不必再交给 uploadFile。 */
export async function readImageBytes(path: string): Promise<Uint8Array> {
  const filePath = path.trim()
  if (!filePath) return Promise.reject({ message: '头像读取失败' })
  const shrunk = await shrinkAvatar(filePath)
  try {
    return await readBytes(shrunk)
  } catch (err) {
    if (shrunk === filePath) throw err
    return readBytes(filePath)
  }
}

function shrinkAvatar(path: string): Promise<string> {
  return new Promise((resolve) => {
    const compress = (uni as unknown as { compressImage?: (opts: object) => void }).compressImage
    if (typeof compress !== 'function') {
      resolve(path)
      return
    }
    compress({
      src: path,
      quality: 80,
      compressedWidth: 512,
      success: (res: { tempFilePath?: string }) => resolve(res.tempFilePath?.trim() || path),
      fail: () => resolve(path),
    })
  })
}

function readBytes(filePath: string): Promise<Uint8Array> {
  const fs = typeof uni.getFileSystemManager === 'function' ? uni.getFileSystemManager() : null
  if (!fs) return readBlobBytes(filePath)
  return new Promise((resolve, reject) => {
    fs.readFile({
      filePath,
      success: (res) => {
        const bytes = toBytes(res.data)
        if (bytes && bytes.byteLength) resolve(bytes)
        else reject({ message: '头像读取失败' })
      },
      fail: () => {
        readBlobBytes(filePath).then(resolve, () => reject({ message: '头像读取失败' }))
      },
    })
  })
}

function readBlobBytes(url: string): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    if (typeof fetch !== 'function') {
      reject({ message: '头像读取失败' })
      return
    }
    fetch(url)
      .then((res) => {
        if (!res.ok) throw new Error('read')
        return res.arrayBuffer()
      })
      .then((buf) => {
        const bytes = new Uint8Array(buf)
        if (!bytes.byteLength) throw new Error('empty')
        resolve(bytes)
      })
      .catch(() => reject({ message: '头像读取失败' }))
  })
}

function userDataDir(): string {
  const fromUni = (uni as unknown as { env?: { USER_DATA_PATH?: string } }).env?.USER_DATA_PATH
  if (fromUni) return fromUni
  const wxEnv = (globalThis as { wx?: { env?: { USER_DATA_PATH?: string } } }).wx?.env?.USER_DATA_PATH
  return wxEnv || ''
}

function toBytes(data: unknown): Uint8Array | null {
  if (data instanceof ArrayBuffer) return new Uint8Array(data)
  if (ArrayBuffer.isView(data)) {
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
  }
  return null
}
