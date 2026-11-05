<script setup lang="ts">
import CreditEmptySheet from '../../components/CreditEmptySheet.vue'
import {
  useCreateStore,
  type BoardSize,
  type PaletteBrand,
  type PalettePref,
} from '../../stores/create'
import { useCreditsStore } from '../../stores/credits'
import { ensureLogin, showError, showTip } from '../../utils/ui'
const create = useCreateStore()
const credits = useCreditsStore()
const sizes: { n: BoardSize; desc: string }[] = [
  { n: 52, desc: '约 2704 钉 · 主力板 · 约 13.5cm' },
  { n: 78, desc: '约 6084 钉 · 常见大板 · 约 20cm' },
  { n: 104, desc: '约 10816 钉 · 超大画幅 · 约 27cm' },
]
const prefs: { id: PalettePref; label: string }[] = [
  { id: 'auto', label: '自动色盘' },
  { id: 'warm', label: '暖色优先' },
  { id: 'macaron', label: '马卡龙' },
  { id: 'contrast', label: '高对比' },
]
const brands: { id: PaletteBrand; label: string }[] = [
  { id: 'generic', label: '通用拼豆' },
  { id: 'mard', label: 'MARD' },
  { id: 'perler', label: 'Perler' },
  { id: 'shop', label: '店内色卡' },
]

function back() {
  uni.navigateBack()
}

function reset() {
  create.resetParams()
  showTip('已重置为推荐参数')
}

function onColor(e: { detail: { value: number } }) {
  create.colorCount = Number(e.detail.value)
}

function generate() {
  if (!ensureLogin()) return
  if (!create.hasImage) {
    showError({ message: '请先选择图片' })
    return
  }
  if (!credits.canGenerate) {
    credits.openEmptySheet()
    return
  }
  uni.navigateTo({ url: '/pages/create/generating' })
}
</script>

<template>
  <view class="page">
    <view class="reset-row">
      <text class="reset" @click="reset">重置</text>
    </view>

    <view class="picked">
      <image v-if="create.localPath" class="thumb" :src="create.localPath" mode="aspectFill" />
      <view v-else class="thumb sample">豆</view>
      <view class="meta">
        <text class="name">{{ create.hasImage ? '参考图已选' : '还没有参考图' }}</text>
        <text class="sub">{{ create.imageLabel || '返回上一步拍照或选相册' }}</text>
        <text class="tag">将走：色块整理→拼豆色号</text>
        <text class="change" @click="back">更换图片</text>
      </view>
    </view>

    <view class="sec-title"><text>稿件名称</text></view>
    <view class="name-card">
      <input
        v-model="create.title"
        class="name-input"
        maxlength="40"
        placeholder="例如：窗边的猫"
        confirm-type="done"
      />
    </view>
    <text class="name-hint">可不填。拍照或相册出稿不填时，会保存为「未命名草稿」。</text>

    <view class="sec-title"><text>珠板规格</text></view>
    <view
      v-for="item in sizes"
      :key="item.n"
      class="option"
      :class="{ on: create.boardSize === item.n }"
      @click="create.boardSize = item.n"
    >
      <view class="txt">
        <text class="strong">{{ item.n }} × {{ item.n }}</text>
        <text class="desc">{{ item.desc }}</text>
      </view>
      <view class="radio" />
    </view>

    <view class="sec-title"><text>色数 / 色盘</text></view>
    <view class="slider-card">
      <view class="row">
        <text>色数上限</text>
        <text class="val">{{ create.colorCount }} 色</text>
      </view>
      <slider
        :min="4"
        :max="24"
        :value="create.colorCount"
        activeColor="#FF6B5B"
        block-size="20"
        @change="onColor"
        @changing="onColor"
      />
      <view class="row hint"><text>更省豆</text><text>更还原</text></view>
    </view>
    <view class="chips">
      <view
        v-for="pref in prefs"
        :key="pref.id"
        class="chip"
        :class="{ on: create.palettePref === pref.id }"
        @click="create.palettePref = pref.id"
      >{{ pref.label }}</view>
    </view>

    <view class="sec-title"><text>色盘品牌</text></view>
    <view class="chips">
      <view
        v-for="item in brands"
        :key="item.id"
        class="chip"
        :class="{ on: create.paletteBrand === item.id }"
        @click="create.paletteBrand = item.id"
      >{{ item.label }}</view>
    </view>
    <text class="name-hint">色号来自内置拼豆 SKU。品牌是色板子集。珠图按上面选定的 52 / 78 / 104 做最近邻，格数以所选珠板为准。</text>

    <view class="note">
      预估消耗 <text class="em">{{ credits.generateCost }}</text> 积分 · 余额
      <text class="em">{{ credits.balance }}</text>（一次性含色块整理与拼豆色号）
    </view>
    <view class="actions">
      <view class="btn ghost" @click="back">上一步</view>
      <view class="btn primary" @click="generate">开始出稿</view>
    </view>
    <text class="demo" @click="credits.openEmptySheet()">演示：积分不足时？</text>
    <CreditEmptySheet />
  </view>
</template>

<style scoped lang="scss">
.page { padding: 12rpx 32rpx 48rpx; }
.reset-row { display: flex; justify-content: flex-end; }
.reset { color: $brand-dark; font-size: 26rpx; font-weight: 650; padding: 8rpx 0 12rpx; }
.picked {
  display: flex;
  gap: 20rpx;
  align-items: center;
  background: $card;
  border: 1rpx solid $line;
  border-radius: 24rpx;
  padding: 20rpx;
}
.thumb { width: 144rpx; height: 144rpx; border-radius: 20rpx; background: $warm-soft; }
.sample {
  display: flex; align-items: center; justify-content: center;
  color: #fff; font-weight: 800; font-size: 40rpx;
  background: linear-gradient(135deg, $brand, $warm);
}
.meta { flex: 1; }
.name { display: block; font-size: 30rpx; font-weight: 700; }
.sub { display: block; margin-top: 6rpx; color: $ink-3; font-size: 24rpx; }
.tag {
  display: inline-block;
  margin-top: 10rpx;
  background: $brand-soft;
  color: $brand-dark;
  border-radius: 999rpx;
  padding: 4rpx 14rpx;
  font-size: 22rpx;
}
.change { display: inline-block; margin-top: 12rpx; margin-left: 12rpx; color: $brand-dark; font-size: 24rpx; font-weight: 650; }
.name-card {
  background: $card;
  border: 1rpx solid $line;
  border-radius: 20rpx;
  padding: 8rpx 24rpx;
}
.name-input { height: 80rpx; font-size: 30rpx; color: $ink; }
.name-hint { display: block; margin-top: 12rpx; color: $ink-3; font-size: 22rpx; line-height: 1.5; }
.option {
  display: flex;
  align-items: center;
  gap: 16rpx;
  background: $card;
  border: 1rpx solid $line;
  border-radius: 20rpx;
  padding: 22rpx 24rpx;
  margin-bottom: 12rpx;
}
.option.on { border-color: $brand; background: $brand-lighter; }
.txt { flex: 1; }
.strong { display: block; font-weight: 750; }
.desc { display: block; margin-top: 4rpx; color: $ink-3; font-size: 22rpx; }
.radio {
  width: 28rpx; height: 28rpx; border-radius: 50%;
  border: 2rpx solid $line;
}
.option.on .radio { border-color: $brand; background: $brand; box-shadow: inset 0 0 0 6rpx #fff; }
.slider-card { background: $card; border-radius: 20rpx; border: 1rpx solid $line; padding: 20rpx 24rpx 8rpx; }
.strength { margin-top: 12rpx; padding-bottom: 20rpx; }
.row { display: flex; justify-content: space-between; align-items: center; font-size: 26rpx; }
.val { color: $brand-dark; font-weight: 800; }
.hint { color: $ink-3; font-size: 22rpx; margin-top: 4rpx; }
.chips { display: flex; flex-wrap: wrap; gap: 12rpx; margin-top: 16rpx; }
.chip {
  padding: 12rpx 24rpx;
  border-radius: 999rpx;
  background: $card;
  border: 1rpx solid $line;
  color: $ink-2;
  font-size: 26rpx;
}
.chip.on { background: $brand-soft; color: $brand-dark; border-color: transparent; font-weight: 700; }
.switch-card {
  display: flex;
  align-items: center;
  gap: 16rpx;
  background: $card;
  border: 1rpx solid $line;
  border-radius: 20rpx;
  padding: 22rpx 24rpx;
}
.switch-txt { flex: 1; }
.switch {
  width: 88rpx;
  height: 52rpx;
  border-radius: 999rpx;
  background: #E4DDD6;
  position: relative;
}
.switch.on { background: $brand; }
.knob {
  width: 44rpx;
  height: 44rpx;
  border-radius: 50%;
  background: #fff;
  position: absolute;
  top: 4rpx;
  left: 4rpx;
}
.switch.on .knob { left: 40rpx; }
.fold {
  margin-top: 20rpx;
  background: $card;
  border: 1rpx solid $line;
  border-radius: 20rpx;
  padding: 22rpx 24rpx;
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.advanced { margin-top: 8rpx; }
.note { margin: 28rpx 0 16rpx; text-align: center; color: $ink-2; font-size: 24rpx; line-height: 1.5; }
.em { color: $ink; font-weight: 800; }
.actions { display: flex; gap: 16rpx; }
.btn { text-align: center; padding: 24rpx 12rpx; border-radius: 999rpx; font-weight: 700; }
.ghost { flex: 1; background: $card; border: 1rpx solid $line; color: $ink; }
.primary { flex: 1.8; background: $brand; color: #fff; }
.demo { display: block; text-align: center; margin-top: 20rpx; color: $ink-3; font-size: 22rpx; }
</style>
