//! 启动钩子。不再写入官方预置稿。
//!
//! 「黄油小熊」「小橘猫吐舌」「蜜桃兔耳」「柠檬汽水」只作为上传示例图的默认标题
//! （见 `drafts::pick_title`），不会落成 `is_official` 稿。教程是小程序内置内容，
//! 作品接口已关闭，都不依赖这些稿件。

use crate::state::AppState;

/// 保留启动入口，但什么都不写。以前会按标题补插四条官方模板，重启后会把已删除的稿写回来。
pub async fn seed_if_empty(_state: &AppState) -> anyhow::Result<()> {
    Ok(())
}
