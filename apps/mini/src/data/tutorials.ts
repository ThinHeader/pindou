export interface TutorialStep {
  title: string
  body: string
}

export interface Tutorial {
  id: string
  title: string
  summary: string
  kicker: string
  cover: string
  tint: string
  steps: TutorialStep[]
}

/** 官方只读教程，不能由用户投稿。 */
export const tutorials: Tutorial[] = [
  {
    id: 'boards',
    title: '三种珠板怎么选',
    summary: '52、78、104 对应不同成品大小。先定底板，再出稿。',
    kicker: '入门 · 约 2 分钟',
    cover: '▦',
    tint: '#FFE8E4',
    steps: [
      {
        title: '52 适合小挂件',
        body: '边长大约 13 厘米，适合钥匙扣和小头像。格子少，第一次拼不容易散。',
      },
      {
        title: '78 适合常规摆件',
        body: '边长大约 20 厘米，五官和简单花纹都能留下来，大多数照片用这一档就够。',
      },
      {
        title: '104 适合想留细节',
        body: '边长大约 26 厘米，用豆更多、耗时更长。照片里的主体要大，背景尽量干净。',
      },
      {
        title: '先选板再出稿',
        body: '在出稿参数里选定珠板和色数。换板需要重新出一张稿，已经保存的稿不会被改掉。',
      },
    ],
  },
  {
    id: 'photo',
    title: '拍照出稿前看一眼',
    summary: '主体清楚、光线均匀，色号才分得开。',
    kicker: '出稿 · 约 2 分钟',
    cover: '📷',
    tint: '#FFF0E6',
    steps: [
      {
        title: '让主体占满画面',
        body: '人、宠物或静物放在画面中间，四周少留杂物。太小的主体出稿后会糊成几块色。',
      },
      {
        title: '避开强逆光和花背景',
        body: '窗边正对的剪影、碎花桌布、其他人的半张脸，都会被算进色号。纯色墙或桌面更稳。',
      },
      {
        title: '一张图只做一个主体',
        body: '合影、拼贴和多件商品会互相抢颜色。想做两只，就分两次出稿。',
      },
      {
        title: '没有照片可以用示例图',
        body: '出稿页提供示例图，用来熟悉珠板、色数和预览，不需要自己准备图片。',
      },
    ],
  },
  {
    id: 'chart',
    title: '色号稿怎么读',
    summary: '预览图对照用豆清单，先看色号，再数颗数。',
    kicker: '读稿 · 约 3 分钟',
    cover: '🎨',
    tint: '#FFF6D8',
    steps: [
      {
        title: '一格就是一颗豆',
        body: '预览图上的每一小格对应珠板上的一个孔。格子的颜色是归并后的色号，不是照片原色。',
      },
      {
        title: '先看用豆清单',
        body: '稿件详情列出每个色号的名称、色值和用量。按用量从多到少备豆，缺得最多的颜色先找齐。',
      },
      {
        title: '色数越少，块面越大',
        body: '色数少，相近的颜色会并成一号，成品更整、更好拼。色数多，过渡更细，也更费时间。',
      },
      {
        title: '导出只是保存图片',
        body: '需要对照着拼时，把预览图保存到相册。这张图只属于你的稿件，不会出现在别人的列表里。',
      },
    ],
  },
  {
    id: 'stitch',
    title: '按颜色往下拼',
    summary: '从用量最多的颜色铺起，一次只拿一种豆。',
    kicker: '开拼 · 约 3 分钟',
    cover: '🧵',
    tint: '#E7F9FC',
    steps: [
      {
        title: '底板先对方向',
        body: '有字的一面朝下，或先在边缘贴一张纸条标出上和左。拼到一半转了向，色块会对不齐。',
      },
      {
        title: '一种颜色铺完再换',
        body: '稿件详情可以按用量打开「开拼」，每次只提示一种色号。铺完再点下一种，桌面上不会同时撒开五六色。',
      },
      {
        title: '大块先连成片',
        body: '背景和衣服这类大面积先连起来，稿不容易在半途散开。眼睛、嘴和细边留到最后点。',
      },
      {
        title: '拼错就停在这一色',
        body: '发现某一色对不上预览时，先不要往上加新颜色。拿掉这一色，对照预览重新铺。',
      },
    ],
  },
  {
    id: 'iron',
    title: '烫平与收边',
    summary: '熨斗中温，隔着熨烫纸，只烫到豆与豆粘住。',
    kicker: '收尾 · 约 3 分钟',
    cover: '♨️',
    tint: '#F3EEFF',
    steps: [
      {
        title: '先烫背面',
        body: '铺好熨烫纸，熨斗垂直落下，不要在豆子上拖。看到孔还在、豆与豆已经粘住，就可以移开。',
      },
      {
        title: '冷却后再翻面',
        body: '刚烫完的板是软的。放平冷却一两分钟，再翻到正面补烫。趁热揭板，孔型会被拉变形。',
      },
      {
        title: '边缘单独看一遍',
        body: '四边和细长的部分最容易虚粘。用镊子轻拉，会松的格子再隔着纸补一下。',
      },
      {
        title: '挂件留出孔位',
        body: '要做钥匙扣时，在烫之前留好挂绳的位置。烫死之后再打孔，容易把周围的豆一起带起来。',
      },
    ],
  },
]

export function tutorialById(id: string): Tutorial | undefined {
  return tutorials.find((item) => item.id === id)
}
