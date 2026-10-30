<script setup lang="ts">
import CreditPill from '../../components/CreditPill.vue'
import CreditEmptySheet from '../../components/CreditEmptySheet.vue'
import { useCreditsStore } from '../../stores/credits'

const credits = useCreditsStore()

function goCreate() {
  uni.switchTab({ url: '/pages/create/index' })
}
function goPlaza() {
  uni.switchTab({ url: '/pages/plaza/index' })
}
function goCredits() {
  uni.navigateTo({ url: '/pages/credits/index' })
}
</script>

<template>
  <view class="page">
    <view class="hero">
      <view class="row">
        <text class="hello">你好，一起拼豆吧</text>
        <CreditPill />
      </view>
      <view class="cta" @click="goCreate">
        <text class="cta-title">拍照出稿</text>
        <text class="cta-sub">消耗 {{ credits.generateCost }} 积分 · 生成色号稿</text>
      </view>
    </view>

    <view class="section">
      <text class="sec-title">快捷入口</text>
      <view class="chips">
        <view class="chip" @click="goCreate">出稿</view>
        <view class="chip" @click="goPlaza">逛广场</view>
        <view class="chip" @click="goCredits">充积分</view>
      </view>
    </view>

    <view class="section">
      <text class="sec-title">今日推荐</text>
      <view class="card muted">设计稿占位 · 推荐作品卡稍后接广场数据</view>
    </view>

    <CreditEmptySheet />
  </view>
</template>

<style scoped lang="scss">
.page { padding: 24rpx 32rpx 120rpx; }
.hero { margin-bottom: 40rpx; }
.row { display: flex; justify-content: space-between; align-items: center; margin-bottom: 28rpx; }
.hello { font-size: 40rpx; font-weight: 700; color: $ink; }
.cta {
  background: linear-gradient(135deg, $brand, $warm);
  border-radius: 32rpx;
  padding: 48rpx 40rpx;
  color: #fff;
  box-shadow: 0 16rpx 40rpx rgba(255, 107, 91, 0.35);
}
.cta-title { display: block; font-size: 40rpx; font-weight: 800; margin-bottom: 8rpx; }
.cta-sub { font-size: 24rpx; opacity: 0.9; }
.section { margin-bottom: 36rpx; }
.sec-title { font-size: 30rpx; font-weight: 650; color: $ink; margin-bottom: 16rpx; display: block; }
.chips { display: flex; gap: 16rpx; flex-wrap: wrap; }
.chip {
  padding: 16rpx 28rpx;
  background: $card;
  border: 1rpx solid $line;
  border-radius: 999rpx;
  color: $ink-2;
  font-size: 26rpx;
}
.card {
  background: $card;
  border-radius: 24rpx;
  padding: 32rpx;
  border: 1rpx solid $line;
}
.muted { color: $ink-3; font-size: 26rpx; }
</style>
