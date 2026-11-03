<script setup lang="ts">
import { ref } from 'vue'
import { useUserStore } from '../../stores/user'
import { showError, showTip } from '../../utils/ui'
import { downloadWechatAvatar, realWechatProfile, requestWechatProfile } from '../../utils/wechat-profile'

const user = useUserStore()
const sheet = ref(false)
const submitting = ref(false)
const nickname = ref('')
const avatarPath = ref('')
const avatarPreview = ref('')

function onChooseAvatar(e: unknown) {
  const path = (e as { detail?: { avatarUrl?: string } }).detail?.avatarUrl?.trim() || ''
  if (!path) return
  avatarPath.value = path
  avatarPreview.value = path
}

function onAvatarTap() {
  // #ifdef H5
  uni.chooseImage({
    count: 1,
    sizeType: ['compressed'],
    sourceType: ['album'],
    success: (res) => {
      const path = res.tempFilePaths?.[0]
      if (!path) return
      avatarPath.value = path
      avatarPreview.value = path
    },
  })
  // #endif
}

function onNickname(e: unknown) {
  const value = (e as { detail?: { value?: string } }).detail?.value
  if (typeof value === 'string') nickname.value = value
}

async function enterWith(name: string, filePath: string) {
  submitting.value = true
  uni.showLoading({ title: '登录中' })
  try {
    await user.login()
    if (name || filePath) await user.saveProfile({ nickname: name, filePath })
    sheet.value = false
    uni.switchTab({ url: '/pages/home/index' })
  } catch (err) {
    showError(err, '登录失败')
  } finally {
    submitting.value = false
    uni.hideLoading()
  }
}

async function wxLogin() {
  if (submitting.value) return
  const info = await requestWechatProfile()
  const real = realWechatProfile(info)
  if (real) {
    try {
      const filePath = await downloadWechatAvatar(real.avatarUrl)
      await enterWith(real.nickName, filePath)
    } catch (err) {
      showError(err, '登录失败')
    }
    return
  }
  // 当前微信不再弹出授权窗，也不再返回真实头像昵称。点登录后再出示填写层。
  sheet.value = true
}

async function allow(e: unknown) {
  if (submitting.value) return
  const fromForm = (e as { detail?: { value?: { nickname?: string } } }).detail?.value?.nickname
  const name = String(typeof fromForm === 'string' ? fromForm : nickname.value).trim()
  if ([...name].length > 20) {
    showTip('昵称需要 1 到 20 个字')
    return
  }
  if (!name || !avatarPath.value) {
    showTip('请授权头像和昵称')
    return
  }
  await enterWith(name, avatarPath.value)
}

function decline() {
  if (submitting.value) return
  void enterWith('', '')
}

function browse() {
  uni.switchTab({ url: '/pages/tutorials/index' })
}
function showLegal() {
  uni.showToast({ title: '协议页即将开放', icon: 'none' })
}
</script>

<template>
  <view class="page">
    <view class="logo">豆</view>
    <text class="title">拼豆</text>
    <text class="tagline">把喜欢的图，变成可拼的色号稿</text>

    <view class="feats">
      <view class="feat"><text class="fico">📷</text><view><text class="ft">一键出稿</text><text class="fs">拍照 / 相册 → 珠板色号图</text></view></view>
      <view class="feat"><text class="fico">▦</text><view><text class="ft">官方教程</text><text class="fs">珠板、色号和烫豆，按步骤看</text></view></view>
      <view class="feat"><text class="fico">🎨</text><view><text class="ft">稿件库</text><text class="fs">模板、收藏、用豆清单随身带</text></view></view>
    </view>

    <button class="btn primary" hover-class="none" :disabled="submitting" @click="wxLogin">微信登录</button>
    <view class="btn ghost" @click="browse">先看教程</view>
    <text class="legal" @click="showLegal">点击微信登录时授权获取头像和昵称，仅在「我的」展示。登录即表示同意《用户协议》与《隐私政策》</text>

    <view v-if="sheet" class="mask">
      <form class="sheet" @submit="allow">
        <text class="sheet-title">授权头像昵称</text>
        <text class="sheet-desc">用于在「我的」展示当前账号</text>
        <button class="avatar-btn" open-type="chooseAvatar" hover-class="none" @chooseavatar="onChooseAvatar" @click="onAvatarTap">
          <image v-if="avatarPreview" class="avatar-img" :src="avatarPreview" mode="aspectFill" />
          <text v-else class="avatar-ph">头像</text>
        </button>
        <input
          class="nick"
          type="nickname"
          name="nickname"
          placeholder="点击填写微信昵称"
          maxlength="20"
          @input="onNickname"
          @blur="onNickname"
        />
        <button class="btn primary sheet-ok" form-type="submit" hover-class="none" :disabled="submitting">允许</button>
        <text class="deny" @click="decline">拒绝</text>
      </form>
    </view>
  </view>
</template>

<style scoped lang="scss">
.page {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 120rpx 48rpx 48rpx;
  background: linear-gradient(180deg, #fff8f5, $bg 42%);
}
.logo {
  width: 140rpx; height: 140rpx; border-radius: 40rpx;
  background: linear-gradient(135deg, $brand, $warm); color: #fff;
  font-size: 60rpx; font-weight: 800; text-align: center; line-height: 140rpx;
  box-shadow: 0 16rpx 36rpx rgba(255, 107, 91, 0.28);
}
.title { margin-top: 24rpx; font-size: 56rpx; font-weight: 800; }
.tagline { margin-top: 8rpx; color: $ink-3; font-size: 28rpx; }
.feats { width: 100%; margin: 48rpx 0 36rpx; display: flex; flex-direction: column; gap: 16rpx; }
.feat { display: flex; gap: 16rpx; align-items: center; background: $card; border-radius: 20rpx; padding: 20rpx; border: 1rpx solid $line; }
.fico { font-size: 36rpx; }
.ft { display: block; font-weight: 750; }
.fs { display: block; color: $ink-3; font-size: 22rpx; margin-top: 4rpx; }
.btn { width: 100%; text-align: center; padding: 26rpx; border-radius: 999rpx; font-weight: 750; margin-bottom: 16rpx; }
.primary { background: $brand; color: #fff; line-height: 1.4; }
.primary::after { border: none; }
.ghost { color: $ink-2; }
.legal { margin-top: 12rpx; font-size: 22rpx; color: $ink-3; line-height: 1.6; text-align: center; }
.mask {
  position: fixed; inset: 0; background: rgba(44, 42, 41, 0.45);
  display: flex; align-items: flex-end; z-index: 20;
}
.sheet {
  width: 100%; background: #fff; border-radius: 32rpx 32rpx 0 0;
  padding: 36rpx 40rpx 48rpx; display: flex; flex-direction: column; align-items: center;
}
.sheet-title, .sheet-desc { display: block; width: 100%; text-align: center; }
.sheet-title { font-size: 34rpx; font-weight: 800; }
.sheet-desc { margin-top: 8rpx; color: $ink-3; font-size: 24rpx; }
.avatar-btn {
  width: 144rpx; height: 144rpx; min-height: 144rpx; padding: 0; margin: 28rpx 0 0;
  border-radius: 40rpx; overflow: hidden; background: $brand-lighter; line-height: 144rpx;
}
.avatar-btn::after { border: none; }
.avatar-ph { color: $brand-dark; font-size: 28rpx; font-weight: 700; }
.avatar-img { width: 144rpx; height: 144rpx; display: block; }
.nick {
  width: 100%; height: 88rpx; margin-top: 24rpx; padding: 0 24rpx;
  background: $bg; border-radius: 20rpx; font-size: 30rpx; text-align: center;
}
.sheet-ok { margin-top: 28rpx; }
.deny { color: $ink-3; font-size: 26rpx; padding: 8rpx 16rpx; }
</style>
