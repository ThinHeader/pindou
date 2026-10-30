<script setup lang="ts">
import CreditPill from '../../components/CreditPill.vue'
import { useCreditsStore } from '../../stores/credits'
import { useUserStore } from '../../stores/user'

const credits = useCreditsStore()
const user = useUserStore()

function goLogin() {
  uni.navigateTo({ url: '/pages/login/index' })
}
function goCredits() {
  uni.navigateTo({ url: '/pages/credits/index' })
}
</script>

<template>
  <view class="page">
    <view class="profile" @click="goLogin">
      <view class="avatar">豆</view>
      <view>
        <text class="name">{{ user.user?.nickname || '点击登录' }}</text>
        <text class="sub">{{ user.isLoggedIn ? '已登录（演示）' : '微信一键登录' }}</text>
      </view>
    </view>

    <view class="card" @click="goCredits">
      <view class="row">
        <text class="t">我的积分</text>
        <CreditPill compact />
      </view>
      <text class="rule">日送 {{ credits.dailyGrant }} · 出稿 −{{ credits.generateCost }} · 广告 +{{ credits.adReward }}</text>
    </view>

    <view class="menu">
      <view class="item" @click="goCredits">充值积分</view>
      <view class="item" @click="credits.claimDaily()">领取今日积分</view>
      <view class="item" @click="uni.switchTab({ url: '/pages/library/index' })">我的稿件库</view>
    </view>
  </view>
</template>

<style scoped lang="scss">
.page { padding: 32rpx; padding-bottom: 120rpx; }
.profile {
  display: flex;
  gap: 24rpx;
  align-items: center;
  margin-bottom: 32rpx;
}
.avatar {
  width: 96rpx; height: 96rpx; border-radius: 28rpx;
  background: linear-gradient(135deg, $brand, $warm);
  color: #fff; font-weight: 800; display: flex; align-items: center; justify-content: center;
}
.name { display: block; font-size: 34rpx; font-weight: 700; }
.sub { font-size: 24rpx; color: $ink-3; }
.card {
  background: $card;
  border-radius: 24rpx;
  padding: 28rpx;
  border: 1rpx solid $line;
  margin-bottom: 24rpx;
}
.row { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12rpx; }
.t { font-weight: 650; }
.rule { font-size: 24rpx; color: $ink-2; }
.menu { background: $card; border-radius: 24rpx; border: 1rpx solid $line; overflow: hidden; }
.item {
  padding: 32rpx 28rpx;
  border-bottom: 1rpx solid $line;
  font-size: 28rpx;
}
.item:last-child { border-bottom: none; }
</style>
