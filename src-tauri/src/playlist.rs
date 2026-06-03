use std::fs;
use std::path::{Path, PathBuf};

/// 支持的视频文件扩展名
const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "ts", "m2ts", "rmvb", "m4v", "mpg", "mpeg",
    "3gp", "ogv",
];

/// 播放列表排序方式
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    Name,
    Size,
}

/// 播放列表条目
#[derive(Clone, Debug, serde::Serialize)]
pub struct PlaylistItem {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_current: bool,
}

#[derive(Clone, Debug)]
struct VideoFile {
    path: PathBuf,
    name: String,
    size: u64,
}

impl VideoFile {
    fn from_path(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        VideoFile { path, name, size }
    }
}

/// 播放列表：维护视频文件路径列表、当前播放索引和排序方式
pub struct Playlist {
    files: Vec<VideoFile>,
    current: usize,
    sort_mode: SortMode,
}

impl Playlist {
    /// 创建空播放列表
    pub fn new() -> Self {
        Playlist {
            files: Vec::new(),
            current: 0,
            sort_mode: SortMode::Name,
        }
    }

    /// 从目录扫描视频文件构建播放列表（递归）
    pub fn from_dir(dir: &Path) -> Result<Self, String> {
        let files = collect_videos(dir)?
            .into_iter()
            .map(VideoFile::from_path)
            .collect();
        let mut playlist = Playlist {
            files,
            current: 0,
            sort_mode: SortMode::Name,
        };
        playlist.sort_by(SortMode::Name);
        Ok(playlist)
    }

    /// 获取当前播放文件路径
    pub fn current_file(&self) -> Option<&PathBuf> {
        self.files.get(self.current).map(|file| &file.path)
    }

    /// 移动到下一个文件，返回下一个文件路径（若有）
    pub fn next(&mut self) -> Option<PathBuf> {
        if self.files.is_empty() {
            return None;
        }
        if self.current + 1 < self.files.len() {
            self.current += 1;
            self.current_file().cloned()
        } else {
            None // 已是最后一个
        }
    }

    /// 移动到上一个文件，返回上一个文件路径（若有）
    pub fn prev(&mut self) -> Option<PathBuf> {
        if self.files.is_empty() || self.current == 0 {
            return None; // 已是第一个
        }
        self.current -= 1;
        self.current_file().cloned()
    }

    /// 切换到指定索引的文件，返回该文件路径
    pub fn set_current(&mut self, index: usize) -> Option<PathBuf> {
        if index >= self.files.len() {
            return None;
        }
        self.current = index;
        self.current_file().cloned()
    }

    /// 删除当前文件，返回被删除的路径；自动调整索引到下一个合法位置
    pub fn remove_current(&mut self) -> Option<PathBuf> {
        if self.files.is_empty() {
            return None;
        }
        let removed = self.files.remove(self.current).path;
        // 调整索引：若删除后 current 越界则退到末尾
        if self.current >= self.files.len() && !self.files.is_empty() {
            self.current = self.files.len() - 1;
        }
        Some(removed)
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    pub fn sort_mode(&self) -> SortMode {
        self.sort_mode
    }

    /// 设置排序方式，并保持当前播放文件不变
    pub fn sort_by(&mut self, mode: SortMode) {
        let current_path = self.current_file().cloned();
        self.files.sort_by(|a, b| match mode {
            SortMode::Name => a
                .name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.path.cmp(&b.path)),
            SortMode::Size => a
                .size
                .cmp(&b.size)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.path.cmp(&b.path)),
        });
        self.sort_mode = mode;

        if let Some(path) = current_path {
            if let Some(index) = self.files.iter().position(|file| file.path == path) {
                self.current = index;
            }
        } else {
            self.current = 0;
        }
    }

    /// 返回播放列表窗口所需的完整条目
    pub fn items(&self) -> Vec<PlaylistItem> {
        self.files
            .iter()
            .enumerate()
            .map(|(index, file)| PlaylistItem {
                name: file.name.clone(),
                path: file.path.to_string_lossy().to_string(),
                size: file.size,
                is_current: index == self.current,
            })
            .collect()
    }

    /// 返回所有文件的文件名列表（用于前端展示）
    pub fn file_names(&self) -> Vec<String> {
        self.files.iter().map(|file| file.name.clone()).collect()
    }
}

/// 递归收集目录下所有视频文件
fn collect_videos(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut result = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("无法读取目录 {:?}: {}", dir, e))?;

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
                VideoFile {
                    path: PathBuf::from("a.mp4"),
                    name: "a.mp4".to_string(),
                    size: 1,
                },
                VideoFile {
                    path: PathBuf::from("b.mp4"),
                    name: "b.mp4".to_string(),
                    size: 2,
                },
                VideoFile {
                    path: PathBuf::from("c.mp4"),
                    name: "c.mp4".to_string(),
                    size: 3,
                },
            ],
            current: 2, // 指向最后一个
            sort_mode: SortMode::Name,
        };
        pl.remove_current(); // 删除 c.mp4，current 应调整到 1
        assert_eq!(pl.current_index(), 1);
        assert_eq!(pl.len(), 2);
    }

    #[test]
    fn test_next_returns_none_at_end() {
        let mut pl = Playlist {
            files: vec![VideoFile {
                path: PathBuf::from("a.mp4"),
                name: "a.mp4".to_string(),
                size: 1,
            }],
            current: 0,
            sort_mode: SortMode::Name,
        };
        assert!(pl.next().is_none());
    }

    #[test]
    fn test_next_advances_index() {
        let mut pl = Playlist {
            files: vec![
                VideoFile {
                    path: PathBuf::from("a.mp4"),
                    name: "a.mp4".to_string(),
                    size: 1,
                },
                VideoFile {
                    path: PathBuf::from("b.mp4"),
                    name: "b.mp4".to_string(),
                    size: 2,
                },
            ],
            current: 0,
            sort_mode: SortMode::Name,
        };
        let next = pl.next();
        assert!(next.is_some());
        assert_eq!(pl.current_index(), 1);
    }

    #[test]
    fn test_sort_by_size_keeps_current_file() {
        let mut pl = Playlist {
            files: vec![
                VideoFile {
                    path: PathBuf::from("large.mp4"),
                    name: "large.mp4".to_string(),
                    size: 100,
                },
                VideoFile {
                    path: PathBuf::from("small.mp4"),
                    name: "small.mp4".to_string(),
                    size: 10,
                },
            ],
            current: 0,
            sort_mode: SortMode::Name,
        };
        pl.sort_by(SortMode::Size);
        assert_eq!(pl.current_file(), Some(&PathBuf::from("large.mp4")));
        assert_eq!(pl.current_index(), 1);
    }

    #[test]
    fn test_set_current_updates_index() {
        let mut pl = Playlist {
            files: vec![
                VideoFile {
                    path: PathBuf::from("a.mp4"),
                    name: "a.mp4".to_string(),
                    size: 1,
                },
                VideoFile {
                    path: PathBuf::from("b.mp4"),
                    name: "b.mp4".to_string(),
                    size: 2,
                },
            ],
            current: 0,
            sort_mode: SortMode::Name,
        };
        assert_eq!(pl.set_current(1), Some(PathBuf::from("b.mp4")));
        assert_eq!(pl.current_index(), 1);
        assert!(pl.set_current(9).is_none());
        assert_eq!(pl.current_index(), 1);
    }
}
