/** 把 chooseAvatar 临时文件读成 base64，供 JSON PUT /api/v1/me。不走 uploadFile。 */

export async function readAvatarBase64(path: string): Promise<string> {
  const filePath = path.trim()
  if (!filePath) return Promise.reject({ message: '头像读取失败' })
  const shrunk = await shrinkAvatar(filePath)
  try {
    return await readBase64(shrunk)
  } catch (err) {
    if (shrunk === filePath) throw err
    return readBase64(filePath)
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

function readBase64(filePath: string): Promise<string> {
  const fs = typeof uni.getFileSystemManager === 'function' ? uni.getFileSystemManager() : null
  if (!fs) return readBlobBase64(filePath)
  return new Promise((resolve, reject) => {
    fs.readFile({
      filePath,
      encoding: 'base64',
      success: (res) => {
        const data = String(res.data ?? '').replace(/\s/g, '')
        if (data) resolve(data)
        else reject({ message: '头像读取失败' })
      },
      fail: () => {
        readBlobBase64(filePath).then(resolve, () => reject({ message: '头像读取失败' }))
      },
    })
  })
}

function readBlobBase64(url: string): Promise<string> {
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
        let binary = ''
        const chunk = 0x8000
        for (let i = 0; i < bytes.length; i += chunk) {
          binary += String.fromCharCode(...bytes.subarray(i, i + chunk))
        }
        resolve(btoa(binary))
      })
      .catch(() => reject({ message: '头像读取失败' }))
  })
}
