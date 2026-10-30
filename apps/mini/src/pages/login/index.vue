<script setup lang="ts">
import { useUserStore } from '../../stores/user'

const user = useUserStore()

async function wxLogin() {
  await user.loginDemo()
  uni.showToast({ title: '登录成功（演示）', icon: 'success' })
  setTimeout(() => uni.switchTab({ url: '/pages/mine/index' }), 400)
}
function guest() {
  uni.switchTab({ url: '/pages/home/index' })
}
</script>

<template>
  <view class="page">
    <view class="logo">豆</view>
    <text class="title">拼豆</text>
    <text class="sub">拍照出稿 · 色号拼豆</text>
    <view class="btn primary" @click="wxLogin">微信一键登录（演示）</view>
    <view class="btn ghost" @click="guest">先逛逛</view>
  </view>
</template>

<style scoped lang="scss">
.page {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48rpx;
  background: linear-gradient(160deg, #FFF8F5, $bg);
}
.logo {
  width: 120rpx; height: 120rpx; border-radius: 32rpx;
  background: linear-gradient(135deg, $brand, $warm);
  color: #fff; font-size: 48rpx; font-weight: 800;
  display: flex; align-items: center; justify-content: center;
  margin-bottom: 24rpx;
  box-shadow: 0 12rpx 32rpx rgba(255, 107, 91, 0.35);
}
.title { font-size: 48rpx; font-weight: 800; color: $ink; }
.sub { color: $ink-3; margin: 12rpx 0 64rpx; font-size: 28rpx; }
.btn {
  width: 100%;
  text-align: center;
  padding: 28rpx;
  border-radius: 999rpx;
  font-weight: 700;
  margin-bottom: 20rpx;
}
.primary { background: $brand; color: #fff; }
.ghost { color: $ink-3; }
</style>
