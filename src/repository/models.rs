//! 剪切板记录的数据模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 剪切板记录的数据模型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipboardRecord {
    /// 唯一标识符（时间戳纳秒）
    pub id: u64,
    /// 剪切板内容
    pub content: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 内容类型
    pub content_type: ContentType,
}

/// 内容类型枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContentType {
    /// 纯文本
    Text,
    /// 图片（存储为 base64）
    Image,
    /// 文件路径
    FilePath,
}
