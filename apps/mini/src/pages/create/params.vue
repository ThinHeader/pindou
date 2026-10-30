<script setup lang="ts">
import { ref } from 'vue'
import CreditEmptySheet from '../../components/CreditEmptySheet.vue'
import { useCreditsStore } from '../../stores/credits'

const credits = useCreditsStore()
const board = ref('52x52')
const colors = ref(24)
const sizes = ['29x29', '52x52', '72x72']

async function generate() {
  const ok = await credits.tryGenerate({
    board_size: board.value,
    color_count: colors.value,
  })
  if (ok) {
    uni.navigateTo({ url: '/pages/create/preview' })
  }
}
</script>

<template>
  <view class="page">
    <text class="label">珠板尺寸</text>
    <view class="row">
      <view
        v-for="s in sizes"
        :key="s"
        class="chip"
        :class="{ on: board === s }"
        @click="board = s"
      >{{ s }}</view>
    </view>
    <text class="label">色数 {{ colors }}</text>
    <slider :min="8" :max="48" :value="colors" activeColor="#FF6B5B" @change="(e: any) => (colors = e.detail.value)" />
    <view class="btn" @click="generate">生成色号稿 · 消耗 {{ credits.generateCost }} 积分</view>
    <CreditEmptySheet />
  </view>
</template>

<style scoped lang="scss">
.page { padding: 32rpx; }
.label { display: block; font-weight: 650; margin: 24rpx 0 16rpx; color: $ink; }
.row { display: flex; gap: 16rpx; flex-wrap: wrap; }
.chip {
  padding: 16rpx 28rpx;
  border-radius: 999rpx;
  background: $card;
  border: 1rpx solid $line;
  color: $ink-2;
}
.chip.on { background: $brand-soft; color: $brand-dark; border-color: transparent; }
.btn {
  margin-top: 64rpx;
  text-align: center;
  padding: 28rpx;
  background: $brand;
  color: #fff;
  border-radius: 999rpx;
  font-weight: 700;
}
</style>
