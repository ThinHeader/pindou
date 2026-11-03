<script setup lang="ts">
import { ref } from 'vue'
import { useUserStore } from '../../stores/user'
import { showError, showTip } from '../../utils/ui'

const user = useUserStore()
const nickname = ref('')
const avatarPath = ref('')
const avatarPreview = ref('')
const submitting = ref(false)

function onChooseAvatar(e: unknown) {
  const path = (e as { detail?: { avatarUrl?: string } }).detail?.avatarUrl?.trim() || ''
  if (!path) return
  avatarPath.value = path
  avatarPreview.value = path
}

function onAvatarTap() {
  // 微信用 open-type="chooseAvatar"。H5 没有该能力，选一张本地图走同一次登录提交。
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

async function wxLogin(e: unknown) {
  if (submitting.value) return
  const fromForm = (e as { detail?: { value?: { nickname?: string } } }).detail?.value?.nickname
  const name = String(typeof fromForm === 'string' ? fromForm : nickname.value).trim()
  if ([...name].length > 20) {
    showTip('昵称需要 1 到 20 个字')
    return
  }
  submitting.value = true
  uni.showLoading({ title: '登录中' })
  try {
    await user.login()
    if (name || avatarPath.value) {
      await user.saveProfile({ nickname: name, filePath: avatarPath.value })
    }
    uni.switchTab({ url: '/pages/home/index' })
  } catch (err) {
    showError(err, '登录失败')
  } finally {
    submitting.value = false
    uni.hideLoading()
  }
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

    <form class="card" @submit="wxLogin">
      <button class="avatar-btn" open-type="chooseAvatar" hover-class="none" @chooseavatar="onChooseAvatar" @click="onAvatarTap">
        <image v-if="avatarPreview" class="avatar-img" :src="avatarPreview" mode="aspectFill" />
        <text v-else class="avatar-ph">选头像</text>
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
      <text class="hint">头像和昵称仅用于在「我的」展示当前账号</text>
      <button class="btn primary" form-type="submit" hover-class="none" :disabled="submitting">微信登录</button>
    </form>

    <view class="btn ghost" @click="browse">先看教程</view>
    <text class="legal" @click="showLegal">登录即表示同意《用户协议》与《隐私政策》</text>
  </view>
</template>

<style scoped lang="scss">
.page {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 96rpx 48rpx 48rpx;
  background: linear-gradient(180deg, #fff8f5, $bg 42%);
}
.logo {
  width: 120rpx; height: 120rpx; border-radius: 36rpx;
  background: linear-gradient(135deg, $brand, $warm); color: #fff;
  font-size: 52rpx; font-weight: 800; text-align: center; line-height: 120rpx;
  box-shadow: 0 16rpx 36rpx rgba(255, 107, 91, 0.28);
}
.title { margin-top: 20rpx; font-size: 52rpx; font-weight: 800; }
.tagline { margin-top: 8rpx; color: $ink-3; font-size: 28rpx; }
.card {
  width: 100%;
  margin-top: 48rpx;
  display: flex;
  flex-direction: column;
  align-items: center;
}
.avatar-btn {
  width: 144rpx; height: 144rpx; min-height: 144rpx; padding: 0; margin: 0;
  border-radius: 40rpx; overflow: hidden; background: $card; line-height: 144rpx;
}
.avatar-btn::after { border: none; }
.avatar-ph {
  display: block; width: 144rpx; height: 144rpx; line-height: 144rpx;
  text-align: center; color: $brand-dark; font-size: 26rpx; font-weight: 700;
  background: $brand-lighter;
}
.avatar-img { width: 144rpx; height: 144rpx; display: block; }
.nick {
  width: 100%; height: 88rpx; margin-top: 28rpx; padding: 0 24rpx;
  background: $card; border: 1rpx solid $line; border-radius: 20rpx;
  font-size: 30rpx; text-align: center;
}
.hint { margin-top: 16rpx; color: $ink-3; font-size: 22rpx; text-align: center; }
.btn { width: 100%; text-align: center; padding: 26rpx; border-radius: 999rpx; font-weight: 750; margin-bottom: 16rpx; }
.primary { margin-top: 28rpx; background: $brand; color: #fff; line-height: 1.4; }
.primary::after { border: none; }
.ghost { color: $ink-2; }
.legal { margin-top: 12rpx; font-size: 22rpx; color: $ink-3; line-height: 1.6; text-align: center; }
</style>
