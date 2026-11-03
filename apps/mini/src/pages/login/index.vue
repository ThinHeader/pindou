<script setup lang="ts">
import { useUserStore } from '../../stores/user'
import { showError } from '../../utils/ui'

const user = useUserStore()

async function wxLogin() {
  uni.showLoading({ title: '登录中' })
  try {
    const data = await user.login()
    uni.showToast({
      title: data.mode === 'wechat' ? '微信登录成功' : '开发态登录成功',
      icon: 'success',
    })
    setTimeout(() => uni.switchTab({ url: '/pages/home/index' }), 400)
  } catch (e) {
    showError(e, '登录失败')
  } finally {
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

    <view class="feats">
      <view class="feat"><text class="fico">📷</text><view><text class="ft">一键出稿</text><text class="fs">拍照 / 相册 → 珠板色号图</text></view></view>
      <view class="feat"><text class="fico">▦</text><view><text class="ft">官方教程</text><text class="fs">珠板、色号和烫豆，按步骤看</text></view></view>
      <view class="feat"><text class="fico">🎨</text><view><text class="ft">稿件库</text><text class="fs">模板、收藏、用豆清单随身带</text></view></view>
    </view>

    <view class="btn primary" @click="wxLogin">微信一键登录</view>
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
.primary { background: $brand; color: #fff; }
.ghost { color: $ink-2; }
.legal { margin-top: 12rpx; font-size: 22rpx; color: $ink-3; line-height: 1.6; text-align: center; }
</style>
