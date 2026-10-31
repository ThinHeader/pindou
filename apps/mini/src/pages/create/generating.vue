<script setup lang="ts">
import { onLoad, onUnload } from '@dcloudio/uni-app'
import { computed, ref, watch } from 'vue'
import CreditEmptySheet from '../../components/CreditEmptySheet.vue'
import { errorOf, jobsApi } from '../../api/http'
import type { JobStage, JobView } from '../../api/types'
import { useCreateStore } from '../../stores/create'
import { useCreditsStore } from '../../stores/credits'
import { showTip } from '../../utils/ui'

const create = useCreateStore()
const credits = useCreditsStore()

const job = ref<JobView | null>(null)
const localError = ref('')
const progress = ref(8)
const started = ref(false)
let stopped = false
let terminal = false
let timer: ReturnType<typeof setTimeout> | null = null
let jobId = ''

const strengthLabel = computed(() => {
  if (create.cartoonStrength === 'light') return '轻'
  if (create.cartoonStrength === 'strong') return '强'
  return '标准'
})

const stages = computed<JobStage[]>(() => {
  if (job.value?.stages?.length) return job.value.stages
  return [
    { id: 'upload', label: '上传原图', state: 'active', detail: '正在上传原图' },
    { id: 'matting', label: '智能去背', state: 'pending', detail: create.removeBg ? '阿里抠图清背景' : '已关闭' },
    { id: 'cartoon', label: '人物动漫化', state: 'pending', detail: `卡通强度 · ${strengthLabel.value}` },
    { id: 'pixel', label: '拼豆量化', state: 'pending', detail: '自研像素 / 色板映射' },
  ]
})

const failed = computed(() => job.value?.status === 'failed' || Boolean(localError.value))
const headline = computed(() => (failed.value ? '这一步没跑通' : '智能处理中…'))
const subline = computed(() => {
  if (localError.value) return localError.value
  if (job.value?.status === 'failed') return job.value.error || job.value.message
  return '预计约 15–30 秒 · 请保持网络畅通'
})
const bar = computed(() => (failed.value ? progress.value : job.value?.progress || progress.value))
const barLabel = computed(() => job.value?.message || (failed.value ? '出稿未完成' : '准备出稿'))

watch(failed, (isFail) => {
  uni.setNavigationBarTitle({ title: isFail ? '出稿未完成' : '正在出稿' })
})

onLoad(() => {
  void start()
})

onUnload(() => {
  stopped = true
  if (timer) clearTimeout(timer)
  if (!terminal && jobId) {
    jobsApi.cancel(jobId).catch(() => undefined)
  }
})

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

async function start() {
  if (started.value && !failed.value) return
  started.value = true
  localError.value = ''
  job.value = null
  terminal = false
  stopped = false
  progress.value = 12
  if (!create.hasImage) {
    localError.value = '请先选择图片。本次未扣积分。'
    terminal = true
    return
  }
  try {
    const created = await create.startJob()
    if (stopped && created) {
      terminal = true
      jobsApi.cancel(created.job_id).catch(() => undefined)
      return
    }
    if (!created) {
      terminal = true
      credits.openEmptySheet()
      setTimeout(() => uni.navigateBack(), 30)
      return
    }
    jobId = created.job_id
    applyJob(created)
    if (!isTerminalStatus(created.status)) schedule()
    else await finish(created)
  } catch (e) {
    const err = errorOf(e)
    terminal = true
    if (err.error === 'insufficient_credits') {
      credits.openEmptySheet()
      setTimeout(() => uni.navigateBack(), 30)
      return
    }
    localError.value = err.message || '出稿失败。本次未扣积分。'
  }
}

function isTerminalStatus(status: string) {
  return status === 'done' || status === 'failed' || status === 'cancelled'
}

function applyJob(next: JobView) {
  job.value = next
  progress.value = next.progress || progress.value
}

function schedule() {
  if (timer) clearTimeout(timer)
  timer = setTimeout(() => void poll(), 700)
}

async function poll() {
  if (stopped || !jobId) return
  try {
    const next = await jobsApi.get(jobId)
    if (stopped) return
    applyJob(next)
    if (isTerminalStatus(next.status)) {
      await finish(next)
      return
    }
    schedule()
  } catch (e) {
    if (stopped) return
    localError.value = errorOf(e).message || '进度查询失败。可稍后在稿件库查看。'
    terminal = true
  }
}

async function finish(next: JobView) {
  terminal = true
  if (next.status === 'done' && next.draft_id) {
    if (next.balance != null) credits.balance = next.balance
    if (next.draft) create.lastDraft = next.draft
    await sleep(650)
    if (stopped) return
    uni.redirectTo({ url: `/pages/create/preview?id=${next.draft_id}` })
    return
  }
  if (next.status === 'cancelled') {
    showTip('已取消，未扣积分')
  }
}

async function cancel() {
  terminal = true
  stopped = true
  if (timer) clearTimeout(timer)
  if (!jobId) {
    uni.navigateBack()
    return
  }
  try {
    const next = await jobsApi.cancel(jobId)
    applyJob(next)
    if (next.status === 'done' && next.draft_id) {
      if (next.balance != null) credits.balance = next.balance
      uni.redirectTo({ url: `/pages/create/preview?id=${next.draft_id}` })
      return
    }
  } catch {
    /* 取消失败也离开页面，服务端仍按未完成处理则不扣分 */
  }
  showTip('已取消，未扣积分')
  uni.navigateBack()
}

function retry() {
  started.value = false
  void start()
}

function changeParams() {
  terminal = true
  stopped = true
  uni.navigateBack()
}

function changeImage() {
  terminal = true
  stopped = true
  uni.navigateBack({
    delta: 2,
    fail: () => uni.switchTab({ url: '/pages/create/index' }),
  })
}

function dot(stage: JobStage, index: number) {
  if (stage.state === 'done' || stage.state === 'skipped') return '✓'
  if (stage.state === 'fail') return '!'
  return String(index + 1)
}
</script>

<template>
  <view class="page">
    <view class="hero" :class="{ fail: failed }">
      <image v-if="create.localPath || job?.original_url" class="thumb" :src="job?.original_url || create.localPath" mode="aspectFill" />
      <view v-else class="thumb sample">豆</view>
      <view v-if="failed" class="bang">!</view>
      <text class="h">{{ headline }}</text>
      <text class="sub">{{ subline }}</text>
    </view>

    <view class="steps">
      <view v-for="(stage, index) in stages" :key="stage.id" class="step" :class="stage.state">
        <text class="dot">{{ dot(stage, index) }}</text>
        <view class="txt">
          <text class="label">{{ stage.label }}</text>
          <text class="detail">{{ stage.detail }}</text>
        </view>
      </view>
    </view>

    <view v-if="!failed" class="bar-wrap">
      <view class="bar"><view class="fill" :style="{ width: bar + '%' }" /></view>
      <view class="meta"><text>进度 {{ bar }}%</text><text>{{ barLabel }}</text></view>
    </view>
    <view v-else class="note">可稍后再试，或调低卡通强度后重新出稿。失败不计费，积分仍保留。</view>

    <view v-if="failed" class="actions">
      <view class="btn primary" @click="retry">重试出稿</view>
      <view class="row">
        <view class="btn ghost" @click="changeParams">换参数</view>
        <view class="btn soft" @click="changeImage">换一张图</view>
      </view>
    </view>
    <view v-else class="actions">
      <view class="btn ghost" @click="cancel">取消出稿</view>
    </view>
    <CreditEmptySheet />
  </view>
</template>

<style scoped lang="scss">
.page { padding: 32rpx 32rpx 48rpx; min-height: 100vh; display: flex; flex-direction: column; }
.hero { text-align: center; padding: 12rpx 0 28rpx; }
.thumb {
  width: 180rpx; height: 180rpx; border-radius: 28rpx; margin: 0 auto 20rpx;
  background: $brand-lighter; display: block;
}
.sample {
  display: flex; align-items: center; justify-content: center;
  color: #fff; font-weight: 800; font-size: 48rpx;
  background: linear-gradient(135deg, $brand, $warm);
}
.bang {
  width: 88rpx; height: 88rpx; border-radius: 50%; margin: 0 auto 16rpx;
  background: #FFF1E8; color: #E07A3D; font-size: 48rpx; font-weight: 800;
  display: flex; align-items: center; justify-content: center;
}
.h { display: block; font-size: 36rpx; font-weight: 800; }
.sub { display: block; margin-top: 10rpx; color: $ink-2; font-size: 24rpx; line-height: 1.5; padding: 0 12rpx; }
.steps { background: $card; border: 1rpx solid $line; border-radius: 24rpx; padding: 8rpx 8rpx; }
.step { display: flex; gap: 16rpx; align-items: flex-start; padding: 18rpx 16rpx; }
.dot {
  width: 44rpx; height: 44rpx; border-radius: 50%; flex-shrink: 0;
  background: $bg; color: $ink-3; font-size: 22rpx; font-weight: 800;
  display: flex; align-items: center; justify-content: center;
}
.step.done .dot, .step.skipped .dot { background: #E7F6EE; color: $success; }
.step.active .dot { background: $brand; color: #fff; }
.step.fail .dot { background: #FFE8E0; color: #E07A3D; }
.txt { flex: 1; }
.label { display: block; font-weight: 750; font-size: 28rpx; }
.detail { display: block; margin-top: 4rpx; color: $ink-3; font-size: 22rpx; }
.step.pending { opacity: 0.55; }
.bar-wrap { margin-top: 28rpx; }
.bar { height: 12rpx; border-radius: 999rpx; background: $line; overflow: hidden; }
.fill { height: 100%; background: $brand; border-radius: 999rpx; }
.meta { display: flex; justify-content: space-between; margin-top: 10rpx; color: $ink-3; font-size: 22rpx; }
.note {
  margin-top: 20rpx;
  background: $brand-lighter;
  border-radius: 20rpx;
  padding: 20rpx 24rpx;
  color: $ink-2;
  font-size: 24rpx;
  line-height: 1.55;
}
.actions { margin-top: auto; padding-top: 36rpx; display: flex; flex-direction: column; gap: 16rpx; }
.row { display: flex; gap: 16rpx; }
.btn { text-align: center; padding: 24rpx; border-radius: 999rpx; font-weight: 700; }
.primary { background: $brand; color: #fff; }
.ghost { background: $card; border: 1rpx solid $line; color: $ink; flex: 1; }
.soft { background: $brand-soft; color: $brand-dark; flex: 1; }
</style>
