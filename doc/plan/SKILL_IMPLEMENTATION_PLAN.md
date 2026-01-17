# Skill System Implementation Plan

> 實施計劃：在 Rust 專案中實現類似 Codex CLI 的 Skill 功能

## 📋 目錄

1. [系統概覽](#系統概覽)
2. [架構設計](#架構設計)
3. [模組劃分](#模組劃分)
4. [實施步驟](#實施步驟)
5. [詳細實施指南](#詳細實施指南)
6. [測試策略](#測試策略)
7. [範例代碼](#範例代碼)

---

## 系統概覽

### 功能目標

構建一個可擴展的技能系統，允許通過檔案系統動態載入和執行專業化工作流程。

### 核心特性

- ✅ **兩階段注入與發現**：Discovery (Brief清單) -> Details (透過工具或指令按需載入)
- ✅ **詳情讀取工具**：提供內建 `get_skill_details` 工具讓 LLM 主動探索技能內容
- ✅ **優先級覆蓋**：同名技能按範圍優先級自動去重
- ✅ **YAML + TOML 配置**：靈活的元資料定義
- ✅ **快取機制**：提升載入效能
- ✅ **非同步載入**：不阻塞主執行緒
- ✅ **錯誤處理**：詳細的錯誤報告和容錯機制

### 技術棧

- **語言**：Rust 2021+
- **關鍵依賴**：
  - `serde` - 序列化/反序列化
  - `tokio` - 非同步運行時
  - `walkdir` - 目錄遍歷
  - `yaml-front-matter` - YAML frontmatter 解析
  - `toml` - TOML 配置解析
  - `thiserror` - 錯誤處理

---

## 架構設計

### 系統架構圖

```
┌─────────────────────────────────────────────────────────────┐
│                        Application                          │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    SkillsManager                            │
│  - Cache management                                         │
│  - Skill discovery coordination                             │
└────────────────────────┬────────────────────────────────────┘
                         │
         ┌───────────────┼───────────────┐
         ▼               ▼               ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   Loader    │  │  Injection  │  │   Render    │
│             │  │             │  │             │
│ - Discovery │  │ - Context   │  │ - Docs      │
│ - Parsing   │  │   injection │  │   generation│
└─────────────┘  └─────────────┘  └─────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│                    File System                              │
│                                                             │
│  Repo: .skills/          User: ~/.myapp/skills/            │
│  System: ~/.myapp/skills/.system/                          │
│  Admin: /etc/myapp/skills/                                 │
└─────────────────────────────────────────────────────────────┘
```

### 資料流向

```
1. 初始化階段
   Application → SkillsManager::new()
                → SystemSkills::install()
                → Extract embedded skills to ~/.myapp/skills/.system/

2. 載入階段
   User requests skills for working directory
                → SkillsManager::skills_for_cwd(cwd)
                → Loader::skill_roots_for_cwd()
                → Loader::load_skills_from_roots()
                → Parse SKILL.md + SKILL.toml
                → Deduplicate by priority
                → Cache results
                → Return SkillLoadOutcome

3. 執行階段
   User selects skill
                → UserInput::Skill { name, path }
                → Injection::build_skill_injections()
                → Load skill file content
                → Inject into agent context
                → Agent executes skill instructions
```

---

## 模組劃分

### 目錄結構

```
src/skills/
├── mod.rs              # 模組匯出
├── model.rs            # 資料結構定義
├── error.rs            # 錯誤類型定義
├── loader.rs           # 技能發現與載入
├── manager.rs          # 快取與管理
├── system.rs           # 系統技能安裝
├── injection.rs        # 上下文注入
├── render.rs           # 文檔渲染
└── tests/
    ├── fixtures/       # 測試用技能範例
    └── integration.rs  # 整合測試
```

### 模組職責

| 模組 | 職責 | 關鍵函數 |
|------|------|----------|
| `model.rs` | 定義資料結構 | `SkillMetadata`, `SkillScope`, `SkillLoadOutcome` |
| `error.rs` | 錯誤類型 | `SkillError`, `Result<T>` |
| `loader.rs` | 發現和解析技能 | `load_skills_from_roots()`, `parse_skill_md()` |
| `manager.rs` | 快取管理 | `SkillsManager::skills_for_cwd()` |
| `system.rs` | 內建技能安裝 | `install_system_skills()` |
| `injection.rs` | 注入到執行上下文 | `build_skill_injections()` |
| `render.rs` | 生成文檔 | `render_skills_section()` |

---

## 實施步驟

### Phase 1: 基礎設施（第 1-2 天）

**目標**：建立基本的資料結構和錯誤處理

- [ ] 步驟 1.1：創建模組結構
- [ ] 步驟 1.2：定義資料模型 (`model.rs`)
- [ ] 步驟 1.3：定義錯誤類型 (`error.rs`)
- [ ] 步驟 1.4：編寫單元測試

### Phase 2: 載入器實現（第 3-5 天）

**目標**：實現技能發現和解析邏輯

- [ ] 步驟 2.1：實現目錄遍歷
- [ ] 步驟 2.2：實現 YAML frontmatter 解析
- [ ] 步驟 2.3：實現 TOML 配置解析
- [ ] 步驟 2.4：實現優先級去重邏輯
- [ ] 步驟 2.5：編寫整合測試

### Phase 3: 管理器與快取（第 6-7 天）

**目標**：實現快取機制和管理器

- [ ] 步驟 3.1：實現 `SkillsManager` 結構
- [ ] 步驟 3.2：實現快取邏輯
- [ ] 步驟 3.3：實現強制重載機制
- [ ] 步驟 3.4：編寫效能測試

### Phase 4: 系統技能（第 8-9 天）

**目標**：實現內建技能管理

- [ ] 步驟 4.1：設計嵌入式技能格式
- [ ] 步驟 4.2：實現技能解壓縮邏輯
- [ ] 步驟 4.3：實現 fingerprinting 避免重複解壓
- [ ] 步驟 4.4：創建範例系統技能

### Phase 5: 注入與渲染（第 10-11 天）

**目標**：實現執行時注入和文檔生成

- [ ] 步驟 5.1：實現 `render_skills_section` (簡短 Brief 清單)
- [ ] 步驟 5.2：實現內建工具 `get_skill_details` 的執行邏輯
- [ ] 步驟 5.3：實現 XML/Markdown 注入格式
- [ ] 步驟 5.4：在 System Prompt 中加入技能使用規範 (Interaction Rules)
- [ ] 步驟 5.5：編寫端到端測試

### Phase 6: 整合與優化（第 12-14 天）

**目標**：整合到主應用並優化

- [ ] 步驟 6.1：與應用主邏輯整合
- [ ] 步驟 6.2：效能分析與優化
- [ ] 步驟 6.3：撰寫使用文檔
- [ ] 步驟 6.4：進行完整測試

---

## 詳細實施指南

### Phase 1: 基礎設施

#### 步驟 1.1：創建模組結構

```bash
mkdir -p src/skills/tests/fixtures
touch src/skills/{mod.rs,model.rs,error.rs,loader.rs,manager.rs,system.rs,injection.rs,render.rs}
touch src/skills/tests/{integration.rs}
```

#### 步驟 1.2：定義資料模型 (`model.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 技能範圍，定義技能的來源和優先級
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SkillScope {
    Admin,  // 優先級最低
    System,
    User,
    Repo,   // 優先級最高
}

/// 技能元資料
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// 技能名稱（唯一標識符）
    pub name: String,

    /// 完整描述
    pub description: String,

    /// 簡短描述（選用）
    pub short_description: Option<String>,

    /// SKILL.md 檔案路徑
    pub path: PathBuf,

    /// 技能範圍
    pub scope: SkillScope,

    /// 介面元資料（來自 SKILL.toml，選用）
    pub interface: Option<SkillInterface>,
}

/// 技能介面元資料（來自 SKILL.toml）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInterface {
    pub display_name: Option<String>,
    pub short_description: Option<String>,
    pub icon_small: Option<String>,
    pub icon_large: Option<String>,
    pub brand_color: Option<String>,
    pub default_prompt: Option<String>,
}

/// SKILL.md frontmatter 結構
#[derive(Debug, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    pub metadata: Option<SkillMetadataSection>,
}

#[derive(Debug, Deserialize)]
pub struct SkillMetadataSection {
    #[serde(rename = "short-description")]
    pub short_description: Option<String>,
}

/// 技能載入結果
#[derive(Debug, Default)]
pub struct SkillLoadOutcome {
    /// 成功載入的技能
    pub skills: Vec<SkillMetadata>,

    /// 載入過程中的錯誤
    pub errors: Vec<crate::skills::error::SkillError>,
}

impl SkillLoadOutcome {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_skill(&mut self, skill: SkillMetadata) {
        self.skills.push(skill);
    }

    pub fn add_error(&mut self, error: crate::skills::error::SkillError) {
        self.errors.push(error);
    }
}
```

#### 步驟 1.3：定義錯誤類型 (`error.rs`)

```rust
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SkillError {
    #[error("Failed to read skill file {path}: {source}")]
    FileRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to parse YAML frontmatter in {path}: {source}")]
    YamlParse {
        path: PathBuf,
        source: serde_yaml::Error,
    },

    #[error("Failed to parse TOML config in {path}: {source}")]
    TomlParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("Missing frontmatter in skill file {path}")]
    MissingFrontmatter { path: PathBuf },

    #[error("Invalid skill name '{name}' in {path}: {reason}")]
    InvalidName {
        name: String,
        path: PathBuf,
        reason: String,
    },

    #[error("Failed to install system skills: {0}")]
    SystemInstall(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SkillError>;
```

---

### Phase 2: 載入器實現

#### 步驟 2.1-2.4：實現載入器 (`loader.rs`)

```rust
use crate::skills::{
    error::{Result, SkillError},
    model::{SkillFrontmatter, SkillInterface, SkillLoadOutcome, SkillMetadata, SkillScope},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// 定義 Repo 內的預設技能存放路徑
const REPO_SKILL_PATHS: &[&str] = &[
    ".skills",
    ".agent/skills",
    ".codex/skills",
    "skills",
];

/// 獲取指定工作目錄的所有技能根目錄
pub fn skill_roots_for_cwd(app_home: &Path, cwd: &Path) -> Vec<(PathBuf, SkillScope)> {
    let mut roots = Vec::new();

    // 1. Repo scope: 從 cwd 向上找專案根目錄標識
    if let Some(repo_root) = find_project_root(cwd) {
        for sub_path in REPO_SKILL_PATHS {
            let repo_skills = repo_root.join(sub_path);
            if repo_skills.exists() && repo_skills.is_dir() {
                roots.push((repo_skills, SkillScope::Repo));
            }
        }
    }

    // 2. User scope: ~/.myapp/skills/
    let user_skills = app_home.join("skills");
    if user_skills.exists() {
        roots.push((user_skills, SkillScope::User));
    }

    // 3. System scope: ~/.myapp/skills/.system/
    let system_skills = app_home.join("skills").join(".system");
    if system_skills.exists() {
        roots.push((system_skills, SkillScope::System));
    }

    // 4. Admin scope: /etc/myapp/skills/ (Unix only)
    #[cfg(unix)]
    {
        let admin_skills = PathBuf::from("/etc/myapp/skills");
        if admin_skills.exists() {
            roots.push((admin_skills, SkillScope::Admin));
        }
    }

    roots
}

/// 從技能根目錄載入所有技能
pub fn load_skills_from_roots(roots: &[(PathBuf, SkillScope)]) -> SkillLoadOutcome {
    let mut outcome = SkillLoadOutcome::new();
    let mut skills_by_name: HashMap<String, SkillMetadata> = HashMap::new();

    for (root, scope) in roots {
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // 只處理 SKILL.md 檔案
            if path.file_name() != Some(std::ffi::OsStr::new("SKILL.md")) {
                continue;
            }

            match parse_skill(path, *scope) {
                Ok(skill) => {
                    // 優先級去重：較高優先級的覆蓋較低優先級的
                    if let Some(existing) = skills_by_name.get(&skill.name) {
                        if skill.scope > existing.scope {
                            skills_by_name.insert(skill.name.clone(), skill);
                        }
                        // 否則保留現有的（優先級更高）
                    } else {
                        skills_by_name.insert(skill.name.clone(), skill);
                    }
                }
                Err(e) => outcome.add_error(e),
            }
        }
    }

    // 將去重後的技能加入結果
    for skill in skills_by_name.into_values() {
        outcome.add_skill(skill);
    }

    outcome
}

/// 解析單個技能檔案
fn parse_skill(path: &Path, scope: SkillScope) -> Result<SkillMetadata> {
    // 讀取檔案內容
    let content = std::fs::read_to_string(path).map_err(|e| SkillError::FileRead {
        path: path.to_path_buf(),
        source: e,
    })?;

    // 解析 YAML frontmatter
    let frontmatter = extract_frontmatter(&content, path)?;

    // 驗證技能名稱
    validate_skill_name(&frontmatter.name, path)?;

    // 嘗試讀取 SKILL.toml（選用）
    let interface = parse_skill_toml(path)?;

    // 提取簡短描述（優先使用 frontmatter 中的）
    let short_description = frontmatter
        .metadata
        .and_then(|m| m.short_description)
        .or_else(|| interface.as_ref()?.short_description.clone());

    Ok(SkillMetadata {
        name: frontmatter.name,
        description: frontmatter.description,
        short_description,
        path: path.to_path_buf(),
        scope,
        interface,
    })
}

/// 提取 YAML frontmatter
fn extract_frontmatter(content: &str, path: &Path) -> Result<SkillFrontmatter> {
    // 檢查是否以 "---" 開頭
    if !content.starts_with("---\n") {
        return Err(SkillError::MissingFrontmatter {
            path: path.to_path_buf(),
        });
    }

    // 找到第二個 "---"
    let rest = &content[4..]; // 跳過第一個 "---\n"
    let end = rest.find("\n---\n").ok_or_else(|| SkillError::MissingFrontmatter {
        path: path.to_path_buf(),
    })?;

    let yaml_str = &rest[..end];

    serde_yaml::from_str(yaml_str).map_err(|e| SkillError::YamlParse {
        path: path.to_path_buf(),
        source: e,
    })
}

/// 解析 SKILL.toml（如果存在）
fn parse_skill_toml(skill_md_path: &Path) -> Result<Option<SkillInterface>> {
    let dir = skill_md_path.parent().unwrap();
    let toml_path = dir.join("SKILL.toml");

    if !toml_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&toml_path).map_err(|e| SkillError::FileRead {
        path: toml_path.clone(),
        source: e,
    })?;

    #[derive(serde::Deserialize)]
    struct SkillToml {
        interface: Option<SkillInterface>,
    }

    let toml: SkillToml = toml::from_str(&content).map_err(|e| SkillError::TomlParse {
        path: toml_path,
        source: e,
    })?;

    Ok(toml.interface)
}

/// 驗證技能名稱
fn validate_skill_name(name: &str, path: &Path) -> Result<()> {
    // 檢查是否為空
    if name.is_empty() {
        return Err(SkillError::InvalidName {
            name: name.to_string(),
            path: path.to_path_buf(),
            reason: "Name cannot be empty".to_string(),
        });
    }

    // 檢查是否包含非法字元（只允許 a-z, 0-9, -, _）
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(SkillError::InvalidName {
            name: name.to_string(),
            path: path.to_path_buf(),
            reason: "Name can only contain a-z, 0-9, -, _".to_string(),
        });
    }

    Ok(())
}

/// 向上查找專案根目錄標識
fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start;
    loop {
        // 檢查多種專案標識
        if current.join(".git").exists() 
            || current.join(".agent").exists() 
            || current.join(".skills").exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_skill_name() {
        let path = Path::new("test.md");

        assert!(validate_skill_name("my-skill", path).is_ok());
        assert!(validate_skill_name("skill_123", path).is_ok());
        assert!(validate_skill_name("", path).is_err());
        assert!(validate_skill_name("my skill", path).is_err());
        assert!(validate_skill_name("my@skill", path).is_err());
    }

    #[test]
    fn test_extract_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill
metadata:
  short-description: Test
---

# Body content
"#;

        let path = Path::new("test.md");
        let fm = extract_frontmatter(content, path).unwrap();

        assert_eq!(fm.name, "test-skill");
        assert_eq!(fm.description, "A test skill");
        assert_eq!(fm.metadata.unwrap().short_description.unwrap(), "Test");
    }
}
```

---

### Phase 3: 管理器與快取

#### 步驟 3.1-3.3：實現管理器 (`manager.rs`)

```rust
use crate::skills::{
    loader::{load_skills_from_roots, skill_roots_for_cwd},
    model::SkillLoadOutcome,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// 技能管理器，負責快取和協調技能載入
pub struct SkillsManager {
    /// 應用主目錄（例如 ~/.myapp/）
    app_home: PathBuf,

    /// 快取：工作目錄 -> 技能載入結果
    cache: Arc<RwLock<HashMap<PathBuf, SkillLoadOutcome>>>,
}

impl SkillsManager {
    /// 創建新的技能管理器
    pub fn new(app_home: PathBuf) -> Self {
        Self {
            app_home,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 獲取指定工作目錄的技能（使用快取）
    pub fn skills_for_cwd(&self, cwd: &Path) -> SkillLoadOutcome {
        self.skills_for_cwd_with_options(cwd, false)
    }

    /// 獲取指定工作目錄的技能（可選擇強制重載）
    pub fn skills_for_cwd_with_options(&self, cwd: &Path, force_reload: bool) -> SkillLoadOutcome {
        let cwd = cwd.to_path_buf();

        // 如果不強制重載，先檢查快取
        if !force_reload {
            if let Ok(cache) = self.cache.read() {
                if let Some(outcome) = cache.get(&cwd) {
                    return outcome.clone();
                }
            }
        }

        // 載入技能
        let roots = skill_roots_for_cwd(&self.app_home, &cwd);
        let outcome = load_skills_from_roots(&roots);

        // 更新快取
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(cwd, outcome.clone());
        }

        outcome
    }

    /// 清除快取
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// 清除特定工作目錄的快取
    pub fn clear_cache_for_cwd(&self, cwd: &Path) {
        if let Ok(mut cache) = self.cache.write() {
            cache.remove(cwd);
        }
    }

    /// 獲取應用主目錄
    pub fn app_home(&self) -> &Path {
        &self.app_home
    }
}

impl Clone for SkillLoadOutcome {
    fn clone(&self) -> Self {
        Self {
            skills: self.skills.clone(),
            errors: vec![], // 不複製錯誤（錯誤通常不實現 Clone）
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cache() {
        let temp = TempDir::new().unwrap();
        let app_home = temp.path().join("app");
        let cwd = temp.path().join("work");

        fs::create_dir_all(&app_home).unwrap();
        fs::create_dir_all(&cwd).unwrap();

        let manager = SkillsManager::new(app_home);

        // 第一次載入
        let outcome1 = manager.skills_for_cwd(&cwd);

        // 第二次應該使用快取
        let outcome2 = manager.skills_for_cwd(&cwd);

        assert_eq!(outcome1.skills.len(), outcome2.skills.len());

        // 清除快取
        manager.clear_cache();

        // 第三次載入
        let outcome3 = manager.skills_for_cwd(&cwd);
        assert_eq!(outcome1.skills.len(), outcome3.skills.len());
    }
}
```

---

### Phase 4: 系統技能

#### 步驟 4.1-4.3：實現系統技能安裝 (`system.rs`)

```rust
use crate::skills::error::{Result, SkillError};
use std::path::Path;

/// 安裝內建系統技能到 ~/.myapp/skills/.system/
pub fn install_system_skills(app_home: &Path) -> Result<()> {
    let system_dir = app_home.join("skills").join(".system");

    // 創建目錄
    std::fs::create_dir_all(&system_dir)?;

    // 檢查 fingerprint，避免重複安裝
    let fingerprint_path = system_dir.join(".fingerprint");
    let current_fingerprint = get_embedded_skills_fingerprint();

    if fingerprint_path.exists() {
        let existing = std::fs::read_to_string(&fingerprint_path)?;
        if existing == current_fingerprint {
            // 已經安裝且版本相同，跳過
            return Ok(());
        }
    }

    // 安裝所有嵌入式技能
    install_embedded_skills(&system_dir)?;

    // 寫入 fingerprint
    std::fs::write(fingerprint_path, current_fingerprint)?;

    Ok(())
}

/// 獲取嵌入式技能的指紋（用於版本檢查）
fn get_embedded_skills_fingerprint() -> String {
    // 可以使用編譯時間戳或內容 hash
    // 這裡簡化為固定版本號
    env!("CARGO_PKG_VERSION").to_string()
}

/// 安裝嵌入式技能
fn install_embedded_skills(target_dir: &Path) -> Result<()> {
    // 示例：安裝一個 "skill-creator" 技能
    let skill_creator_dir = target_dir.join("skill-creator");
    std::fs::create_dir_all(&skill_creator_dir)?;

    let skill_md = include_str!("../assets/system_skills/skill-creator/SKILL.md");
    std::fs::write(skill_creator_dir.join("SKILL.md"), skill_md)?;

    // 可以添加更多系統技能...

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_install_system_skills() {
        let temp = TempDir::new().unwrap();
        let app_home = temp.path();

        // 第一次安裝
        install_system_skills(app_home).unwrap();

        let system_dir = app_home.join("skills").join(".system");
        assert!(system_dir.exists());
        assert!(system_dir.join(".fingerprint").exists());

        // 第二次安裝應該跳過
        install_system_skills(app_home).unwrap();
    }
}
```

在 `src/assets/system_skills/skill-creator/SKILL.md` 中創建範例：

```markdown
---
name: skill-creator
description: Guide for creating new skills in your application
metadata:
  short-description: Create or update a skill
---

# Skill Creator

This skill helps you create new skills for your application.

## Skill Structure

Each skill requires:
1. A directory with the skill name (e.g., `my-skill/`)
2. A `SKILL.md` file with YAML frontmatter
3. Optional `SKILL.toml` for enhanced metadata

## Example SKILL.md

\```markdown
---
name: my-skill
description: What this skill does
metadata:
  short-description: Short description
---

# Skill Instructions

Your skill instructions go here.
\```

## Where to Place Skills

- **Project skills**: `.skills/` in repository root
- **User skills**: `~/.myapp/skills/`
- **System skills**: `~/.myapp/skills/.system/` (auto-installed)
```

---

### Phase 5: 注入與渲染

#### 步驟 5.1-5.2：實現注入邏輯 (`injection.rs`)

```rust
use crate::skills::{error::Result, model::{SkillLoadOutcome, SkillMetadata}};
use std::path::PathBuf;

/// 使用者輸入類型
#[derive(Debug, Clone)]
pub enum UserInput {
    Text { text: String },
    /// 顯示激發（例如透過 Slash Command 或 UI 選擇）
    Skill { name: String, path: PathBuf },
}

/// 技能注入結果
#[derive(Debug)]
pub struct SkillInjections {
    pub injections: Vec<SkillInjection>,
}

#[derive(Debug)]
pub struct SkillInjection {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
}

/// 構建技能注入（通常用於顯示激發，直接讀取全文）
pub async fn build_skill_injections(
    inputs: &[UserInput],
) -> Result<SkillInjections> {
    let mut injections = Vec::new();

    for input in inputs {
        if let UserInput::Skill { name, path } = input {
            let content = tokio::fs::read_to_string(path).await.map_err(|e| SkillError::FileRead {
                path: path.clone(),
                source: e,
            })?;

            injections.push(SkillInjection {
                name: name.clone(),
                path: path.clone(),
                content,
            });
        }
    }

    Ok(SkillInjections { injections })
}

/// 專用的 get_skill_details 工具邏輯
pub async fn get_skill_details(name: &str, outcome: &SkillLoadOutcome) -> Result<String> {
    let skill = outcome.skills.iter()
        .find(|s| s.name == name)
        .ok_or_else(|| SkillError::InvalidName { 
            name: name.to_string(), 
            path: PathBuf::new(), 
            reason: "Skill not found".to_string() 
        })?;

    let content = tokio::fs::read_to_string(&skill.path).await.map_err(|e| SkillError::FileRead {
        path: skill.path.clone(),
        source: e,
    })?;
    
    // 移除 YAML frontmatter，只給 LLM 指令部分
    Ok(strip_frontmatter(&content))
}

fn strip_frontmatter(content: &str) -> String {
    if content.starts_with("---\n") {
        if let Some(end) = content[4..].find("\n---\n") {
            return content[4 + end + 5..].to_string();
        }
    }
    content.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_skill_injections() {
        use tempfile::NamedTempFile;
        use std::io::Write;

        let mut temp = NamedTempFile::new().unwrap();
        write!(temp, "Skill content here").unwrap();

        let inputs = vec![UserInput::Skill {
            name: "test-skill".to_string(),
            path: temp.path().to_path_buf(),
        }];

        let result = build_skill_injections(&inputs, None).await.unwrap();

        assert_eq!(result.injections.len(), 1);
        assert!(result.injections[0].content.contains("<skill>"));
        assert!(result.injections[0].content.contains("test-skill"));
    }
}
```

#### 步驟 5.3：實現文檔渲染 (`render.rs`)

```rust
use crate::skills::model::SkillLoadOutcome;

/// 渲染技能清單為 Markdown Brief (用於 System Prompt)
pub fn render_skills_brief(outcome: &SkillLoadOutcome) -> String {
    if outcome.skills.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    output.push_str("## Available Skills\n");
    output.push_str("The following skills are available. To use a skill, you MUST call `get_skill_details` to read its full instructions first.\n\n");

    for skill in &outcome.skills {
        let desc = skill.short_description.as_ref().unwrap_or(&skill.description);
        output.push_str(&format!("- **{}**: {} (path: `{}`)\n", 
            skill.name, desc, skill.path.display()));
    }
    
    output.push_str("\n**Skill Usage Rules**:\n");
    output.push_str("1. If a skill description matches the user's task, use it.\n");
    output.push_str("2. Always read the detailed instructions via `get_skill_details` before execution.\n");
    output.push_str("3. Only load the specific reference files if requested by the skill instructions.\n");

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::model::{SkillMetadata, SkillScope};
    use std::path::PathBuf;

    #[test]
    fn test_render_skills_section() {
        let mut outcome = SkillLoadOutcome::new();
        outcome.add_skill(SkillMetadata {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            short_description: Some("Test".to_string()),
            path: PathBuf::from("/test/SKILL.md"),
            scope: SkillScope::User,
            interface: None,
        });

        let rendered = render_skills_section(&outcome);

        assert!(rendered.contains("# Available Skills"));
        assert!(rendered.contains("## test-skill"));
        assert!(rendered.contains("**Test**"));
        assert!(rendered.contains("A test skill"));
    }
}
```

---

### Phase 6: 模組匯出

#### `mod.rs`

```rust
pub mod error;
pub mod injection;
pub mod loader;
pub mod manager;
pub mod model;
pub mod render;
pub mod system;

pub use error::{Result, SkillError};
pub use injection::{build_skill_injections, SkillInjections, UserInput};
pub use loader::{load_skills_from_roots, skill_roots_for_cwd};
pub use manager::SkillsManager;
pub use model::{SkillLoadOutcome, SkillMetadata, SkillScope};
pub use render::render_skills_section;
pub use system::install_system_skills;
```

---

## 測試策略

### 單元測試

每個模組都應包含單元測試：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_x() {
        // 測試邏輯
    }
}
```

### 整合測試

在 `tests/integration.rs` 中：

```rust
use tempfile::TempDir;
use std::fs;

#[test]
fn test_end_to_end_skill_loading() {
    // 1. 創建臨時目錄結構
    let temp = TempDir::new().unwrap();
    let app_home = temp.path().join("app");
    let repo = temp.path().join("repo");

    // 2. 創建測試技能
    let user_skills = app_home.join("skills");
    fs::create_dir_all(&user_skills).unwrap();

    let skill_dir = user_skills.join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    let skill_content = r#"---
name: test-skill
description: A test skill
---

# Test Skill Body
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_content).unwrap();

    // 3. 初始化管理器
    let manager = myapp::skills::SkillsManager::new(app_home);

    // 4. 載入技能
    let outcome = manager.skills_for_cwd(&repo);

    // 5. 驗證結果
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(outcome.skills[0].name, "test-skill");
}
```

### 效能測試

```rust
#[test]
fn bench_load_many_skills() {
    use std::time::Instant;

    // 創建 100 個技能
    // ...

    let start = Instant::now();
    let outcome = manager.skills_for_cwd(&cwd);
    let duration = start.elapsed();

    assert!(duration.as_millis() < 100, "Loading should be fast");
}
```

---

## 範例代碼

### 完整使用範例

```rust
use myapp::skills::{SkillsManager, install_system_skills};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 初始化
    let app_home = dirs::home_dir()
        .unwrap()
        .join(".myapp");

    // 2. 安裝系統技能
    install_system_skills(&app_home)?;

    // 3. 創建管理器
    let manager = SkillsManager::new(app_home);

    // 4. 獲取當前目錄的技能
    let cwd = std::env::current_dir()?;
    let outcome = manager.skills_for_cwd(&cwd);

    // 5. 顯示技能
    println!("Found {} skills:", outcome.skills.len());
    for skill in &outcome.skills {
        println!("  - {} ({})", skill.name, skill.description);
    }

    // 6. 模擬技能執行
    if let Some(skill) = outcome.skills.first() {
        let inputs = vec![myapp::skills::UserInput::Skill {
            name: skill.name.clone(),
            path: skill.path.clone(),
        }];

        let injections = myapp::skills::build_skill_injections(&inputs, Some(&outcome)).await?;

        println!("\nSkill injection:");
        println!("{}", injections.injections[0].content);
    }

    Ok(())
}
```

---

## 依賴項配置

在 `Cargo.toml` 中添加：

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
toml = "0.8"
tokio = { version = "1.0", features = ["fs", "rt-multi-thread"] }
walkdir = "2.4"
thiserror = "1.0"
dirs = "5.0"

[dev-dependencies]
tempfile = "3.8"
pretty_assertions = "1.4"
```

---

## 檢查清單

### 實施前

- [ ] 確認 Rust 版本 >= 1.70
- [ ] 了解專案整體架構
- [ ] 規劃技能目錄結構

### Phase 1

- [ ] 創建模組結構
- [ ] 實現 `model.rs`
- [ ] 實現 `error.rs`
- [ ] 編寫單元測試

### Phase 2

- [ ] 實現目錄遍歷
- [ ] 實現 YAML 解析
- [ ] 實現 TOML 解析
- [ ] 實現優先級邏輯
- [ ] 編寫整合測試

### Phase 3

- [ ] 實現 `SkillsManager`
- [ ] 實現快取邏輯
- [ ] 測試快取效能

### Phase 4

- [ ] 設計系統技能格式
- [ ] 實現安裝邏輯
- [ ] 創建範例技能

### Phase 5

- [ ] 實現非同步注入
- [ ] 實現文檔渲染
- [ ] 端到端測試

### Phase 6

- [ ] 整合到主應用
- [ ] 效能優化
- [ ] 撰寫文檔
- [ ] 完整測試

---

## 擴展點

### 未來可增強功能

1. **技能版本管理**
   - 支援技能版本號
   - 自動更新機制

2. **技能依賴**
   - 技能可聲明依賴其他技能
   - 自動載入依賴鏈

3. **熱重載**
   - 監聽檔案變更
   - 自動重載技能

4. **技能市場**
   - 遠端技能倉庫
   - 一鍵安裝機制

5. **權限控制**
   - 技能執行權限管理
   - 沙箱隔離

---

## 參考資源

- [Codex CLI Source Code](https://github.com/codex-cli/codex)
- [Serde Documentation](https://serde.rs/)
- [Tokio Documentation](https://tokio.rs/)
- [WalkDir Crate](https://docs.rs/walkdir/)

---

**版本**: 1.0
**最後更新**: 2026-01-16
**作者**: Claude Code Implementation Plan Generator
