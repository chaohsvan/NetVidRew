use std::fs;
use std::path::{Path, PathBuf};

/// 支持的视频文件扩展名
const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "ts", "m2ts", "rmvb", "m4v", "mpg",
    "mpeg", "3gp", "ogv",
];

/// 播放列表：维护视频文件路径列表和当前播放索引
pub struct Playlist {
    files: Vec<PathBuf>,
    current: usize,
}

impl Playlist {
    /// 创建空播放列表
    pub fn new() -> Self {
        Playlist {
            files: Vec::new(),
            current: 0,
        }
    }

    /// 从目录扫描视频文件构建播放列表（递归）
    pub fn from_dir(dir: &Path) -> Result<Self, String> {
        let mut files = collect_videos(dir)?;
        files.sort_by(|a, b| {
            a.file_name()
                .unwrap_or_default()
                .cmp(b.file_name().unwrap_or_default())
        });
        Ok(Playlist { files, current: 0 })
    }

    /// 获取当前播放文件路径
    pub fn current_file(&self) -> Option<&PathBuf> {
        self.files.get(self.current)
    }

    /// 移动到下一个文件，返回下一个文件路径（若有）
    pub fn next(&mut self) -> Option<&PathBuf> {
        if self.files.is_empty() {
            return None;
        }
        if self.current + 1 < self.files.len() {
            self.current += 1;
            self.files.get(self.current)
        } else {
            None // 已是最后一个
        }
    }

    /// 删除当前文件，返回被删除的路径；自动调整索引到下一个合法位置
    pub fn remove_current(&mut self) -> Option<PathBuf> {
        if self.files.is_empty() {
            return None;
        }
        let removed = self.files.remove(self.current);
        // 调整索引：若删除后 current 越界则退到末尾
        if self.current >= self.files.len() && !self.files.is_empty() {
            self.current = self.files.len() - 1;
        }
        Some(removed)
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    /// 返回所有文件的文件名列表（用于前端展示）
    pub fn file_names(&self) -> Vec<String> {
        self.files
            .iter()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect()
    }
}

/// 递归收集目录下所有视频文件
fn collect_videos(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut result = Vec::new();
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("无法读取目录 {:?}: {}", dir, e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();
        if path.is_dir() {
            // 递归子目录
            if let Ok(mut sub) = collect_videos(&path) {
                result.append(&mut sub);
            }
        } else if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            if VIDEO_EXTENSIONS.contains(&ext_lower.as_str()) {
                result.push(path);
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_playlist_is_empty() {
        let pl = Playlist::new();
        assert!(pl.is_empty());
        assert_eq!(pl.len(), 0);
        assert!(pl.current_file().is_none());
    }

    #[test]
    fn test_remove_current_adjusts_index() {
        let mut pl = Playlist {
            files: vec![
                PathBuf::from("a.mp4"),
                PathBuf::from("b.mp4"),
                PathBuf::from("c.mp4"),
            ],
            current: 2, // 指向最后一个
        };
        pl.remove_current(); // 删除 c.mp4，current 应调整到 1
        assert_eq!(pl.current_index(), 1);
        assert_eq!(pl.len(), 2);
    }

    #[test]
    fn test_next_returns_none_at_end() {
        let mut pl = Playlist {
            files: vec![PathBuf::from("a.mp4")],
            current: 0,
        };
        assert!(pl.next().is_none());
    }

    #[test]
    fn test_next_advances_index() {
        let mut pl = Playlist {
            files: vec![PathBuf::from("a.mp4"), PathBuf::from("b.mp4")],
            current: 0,
        };
        let next = pl.next();
        assert!(next.is_some());
        assert_eq!(pl.current_index(), 1);
    }
}
