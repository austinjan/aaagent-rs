# Skill System Implementation Plan

> 實施計劃：在 Rust 專案中實現類似 Codex CLI 的 Skill 功能
> Updated: 2026-01-31 - Incorporated improvements from OpenClaw implementation

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

- ✅ **兩階段注入與發現**：Discovery (Brief XML清單) -> Details (透過 Read 工具按需載入)
- ✅ **使用現有 Read 工具**：LLM 使用已知的 Read 工具讀取 SKILL.md 全文
- ✅ **資格過濾**：根據二進制檔案、環境變數、配置、作業系統過濾技能
- ✅ **調用控制**：user-invocable 和 disable-model-invocation 標誌
- ✅ **優先級覆蓋**：同名技能按範圍優先級自動去重
- ✅ **YAML + TOML 配置**：靈活的元資料定義
- ✅ **Per-skill 配置**：用戶可在 config.yaml 中配置個別技能
- ✅ **環境變數注入**：自動注入 API 金鑰到技能指定的環境變數
- ✅ **即時載入**：每次請求重新掃描技能以支援動態修改
- ✅ **非同步載入**：不阻塞主執行緒
- ✅ **錯誤處理**：錯誤訊息簡單清楚，輸出到 Logs 並回傳 API

### 產品決策與假設

- **同名衝突**：依據預設優先順序覆寫（以 Scope 優先級為準）
- **動態修改**：提供手動 reload API，不做自動 watcher（MVP）
- **目標規模**：50~100 skills；不加內建觀測指標
- **使用情境**：Web application，提供 API 取代 CLI
- **安全策略**：MVP 允許所有路徑與符號連結
- **可觀測性**：skill 被調用需記錄並透過 streaming 同步到 UI

### 技術棧

- **語言**：Rust 2021+
- **關鍵依賴**：
  - `serde` - 序列化/反序列化
  - `tokio` - 非同步運行時
  - `walkdir` - 目錄遍歷
  - `serde_yaml` - YAML frontmatter 解析
  - `toml` - TOML 配置解析
  - `thiserror` - 錯誤處理
  - `which` - 二進制檔案檢測

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
│  - Skill discovery coordination                             │
│  - Per-skill config resolution                              │
└────────────────────────┬────────────────────────────────────┘
                         │
         ┌───────────────┼───────────────┬───────────────┐
         ▼               ▼               ▼               ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   Loader    │  │  Eligibility│  │   Render    │  │  Env        │
│             │  │   Filter    │  │             │  │  Injection  │
│ - Discovery │  │ - bins/env  │  │ - XML list  │  │             │
│ - Parsing   │  │ - OS/config │  │ - Sys prompt│  │ - API keys  │
└─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│                    File System                              │
│                                                             │
│  Repo: .skills/          User: ~/.aaagent/skills/          │
│  System: ~/.aaagent/skills/.system/                        │
│  Admin: /etc/aaagent/skills/                               │
└─────────────────────────────────────────────────────────────┘
```

### 資料流向

```
1. 初始化階段
   Application → SkillsManager::new()
                → SystemSkills::install()
                → Extract embedded skills to ~/.aaagent/skills/.system/

2. 載入階段
   User/UI requests skills via API (list/reload)
                → SkillsManager::skills_for_cwd(cwd)
                → Loader::skill_roots_for_cwd()
                → Loader::load_skills_from_roots()
                → Parse SKILL.md + SKILL.toml
                → Deduplicate by priority
                → Filter by eligibility
                → Return SkillSnapshot

3. 執行階段
   Agent starts
                → Apply environment overrides (inject API keys)
                → Build system prompt with XML skill list
                → LLM scans <available_skills>
                → LLM calls Read tool to fetch SKILL.md content
                → LLM follows skill instructions
                → Cleanup: restore environment
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
├── eligibility.rs      # 資格過濾邏輯
├── manager.rs          # 組裝快照與管理
├── system.rs           # 系統技能安裝
├── config.rs           # Per-skill 配置解析
├── env_override.rs     # 環境變數注入
├── render.rs           # XML 渲染
└── tests/
    ├── fixtures/       # 測試用技能範例
    └── integration.rs  # 整合測試
```

### 模組職責

| 模組 | 職責 | 關鍵函數 |
|------|------|----------|
| `model.rs` | 定義資料結構 | `SkillMetadata`, `SkillScope`, `SkillSnapshot`, `SkillRequirements` |
| `error.rs` | 錯誤類型 | `SkillError`, `Result<T>` |
| `loader.rs` | 發現和解析技能 | `load_skills_from_roots()`, `parse_skill_md()` |
| `eligibility.rs` | 資格過濾 | `filter_eligible_skills()`, `check_requirements()` |
| `manager.rs` | 快照組裝 | `SkillsManager::build_snapshot()` |
| `system.rs` | 內建技能安裝 | `install_system_skills()` |
| `config.rs` | Per-skill 配置 | `resolve_skill_config()` |
| `env_override.rs` | 環境變數注入 | `apply_env_overrides()`, `restore_env()` |
| `render.rs` | 生成 XML | `render_skills_xml()` |

---

## 實施步驟

### Phase 1: 基礎設施（第 1-2 天）

**目標**：建立基本的資料結構和錯誤處理

- [ ] 步驟 1.1：創建模組結構
- [ ] 步驟 1.2：定義資料模型 (`model.rs`) - 包含 SkillRequirements, SkillInvocation
- [ ] 步驟 1.3：定義錯誤類型 (`error.rs`)
- [ ] 步驟 1.4：編寫單元測試

### Phase 2: 載入器實現（第 3-5 天）

**目標**：實現技能發現和解析邏輯

- [ ] 步驟 2.1：實現目錄遍歷
- [ ] 步驟 2.2：實現 YAML frontmatter 解析（包含 metadata JSON5）
- [ ] 步驟 2.3：實現 TOML 配置解析
- [ ] 步驟 2.4：實現優先級去重邏輯
- [ ] 步驟 2.5：編寫整合測試

### Phase 3: 資格過濾（第 6-7 天）

**目標**：實現技能資格檢查

- [ ] 步驟 3.1：實現二進制檔案檢測 (`which` crate)
- [ ] 步驟 3.2：實現環境變數檢查
- [ ] 步驟 3.3：實現配置路徑檢查
- [ ] 步驟 3.4：實現作業系統過濾
- [ ] 步驟 3.5：編寫過濾邏輯測試

### Phase 4: 管理器與快照（第 8-9 天）

**目標**：實現快照組裝與管理器

- [ ] 步驟 4.1：實現 `SkillsManager` 結構
- [ ] 步驟 4.2：實現 `build_snapshot()` 方法
- [ ] 步驟 4.3：編寫效能測試

### Phase 5: Per-skill 配置（第 10-11 天）

**目標**：實現用戶配置解析

- [ ] 步驟 5.1：定義 config.yaml 中的 skills 區段格式
- [ ] 步驟 5.2：實現 `resolve_skill_config()`
- [ ] 步驟 5.3：實現 enabled/disabled 過濾
- [ ] 步驟 5.4：編寫配置測試

### Phase 6: 環境變數注入（第 12-13 天）

**目標**：實現 API 金鑰注入

- [ ] 步驟 6.1：實現 `apply_env_overrides()`
- [ ] 步驟 6.2：實現 `restore_env()` 清理函數
- [ ] 步驟 6.3：實現 primaryEnv → apiKey 映射
- [ ] 步驟 6.4：編寫注入測試

### Phase 7: 系統技能（第 14-15 天）

**目標**：實現內建技能管理

- [ ] 步驟 7.1：設計嵌入式技能格式
- [ ] 步驟 7.2：實現技能解壓縮邏輯
- [ ] 步驟 7.3：實現 fingerprinting 避免重複解壓
- [ ] 步驟 7.4：創建範例系統技能

### Phase 8: XML 渲染與系統提示（第 16-17 天）

**目標**：實現 XML 格式和系統提示集成

- [ ] 步驟 8.1：實現 `render_skills_xml()` (XML 格式)
- [ ] 步驟 8.2：實現系統提示技能區段
- [ ] 步驟 8.3：實現調用控制過濾 (disable-model-invocation)
- [ ] 步驟 8.4：編寫端到端測試

### Phase 9: 整合與優化（第 18-20 天）

**目標**：整合到主應用並優化

- [ ] 步驟 9.1：與 Agent 主邏輯整合
- [ ] 步驟 9.2：提供 Web API（list skills / get skill details / reload）
- [ ] 步驟 9.3：技能調用事件記錄並透過 streaming 提供給 UI
- [ ] 步驟 9.4：效能分析與優化
- [ ] 步驟 9.5：撰寫使用文檔
- [ ] 步驟 9.6：進行完整測試

---

## 詳細實施指南

### Phase 1: 基礎設施

#### 步驟 1.1：創建模組結構

```bash
mkdir -p src/skills/tests/fixtures
touch src/skills/{mod.rs,model.rs,error.rs,loader.rs,eligibility.rs,manager.rs,system.rs,config.rs,env_override.rs,render.rs}
touch src/skills/tests/{integration.rs}
```

#### 步驟 1.2：定義資料模型 (`model.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// 技能範圍，定義技能的來源和優先級
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SkillScope {
    Admin,  // 優先級最低 (/etc/aaagent/skills/)
    System, // ~/.aaagent/skills/.system/
    User,   // ~/.aaagent/skills/
    Repo,   // 優先級最高 (.skills/ in project)
}

/// 技能資格要求（來自 metadata.requires）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillRequirements {
    /// 所有必須存在的二進制檔案
    #[serde(default)]
    pub bins: Vec<String>,

    /// 至少一個必須存在的二進制檔案
    #[serde(default, rename = "anyBins")]
    pub any_bins: Vec<String>,

    /// 必須存在的環境變數
    #[serde(default)]
    pub env: Vec<String>,

    /// 必須為真的配置路徑（點分隔）
    #[serde(default)]
    pub config: Vec<String>,
}

/// 技能調用控制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInvocation {
    /// 用戶可透過 slash command 調用（預設 true）
    #[serde(default = "default_true")]
    pub user_invocable: bool,

    /// 對模型隱藏（預設 false）
    #[serde(default)]
    pub disable_model_invocation: bool,
}

fn default_true() -> bool { true }

impl Default for SkillInvocation {
    fn default() -> Self {
        Self {
            user_invocable: true,
            disable_model_invocation: false,
        }
    }
}

/// 技能元資料（來自 metadata 欄位的 JSON5）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillOpenClawMetadata {
    /// 總是可用（跳過資格檢查）
    #[serde(default)]
    pub always: bool,

    /// 備用配置查找鍵
    #[serde(rename = "skillKey")]
    pub skill_key: Option<String>,

    /// 主要環境變數（用於 API 金鑰注入）
    #[serde(rename = "primaryEnv")]
    pub primary_env: Option<String>,

    /// UI 表情符號
    pub emoji: Option<String>,

    /// 文檔 URL
    pub homepage: Option<String>,

    /// 支援的作業系統
    #[serde(default)]
    pub os: Vec<String>,

    /// 資格要求
    #[serde(default)]
    pub requires: SkillRequirements,
}

/// 技能元資料
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// 技能名稱（唯一標識符）
    pub name: String,

    /// 完整描述
    pub description: String,

    /// SKILL.md 檔案路徑
    pub path: PathBuf,

    /// 技能範圍
    pub scope: SkillScope,

    /// 調用控制
    #[serde(default)]
    pub invocation: SkillInvocation,

    /// OpenClaw 風格元資料
    pub openclaw_metadata: Option<SkillOpenClawMetadata>,

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

    /// JSON5 編碼的元資料
    pub metadata: Option<String>,

    /// 用戶可調用（預設 true）
    #[serde(default = "default_true", rename = "user-invocable")]
    pub user_invocable: bool,

    /// 禁用模型調用（預設 false）
    #[serde(default, rename = "disable-model-invocation")]
    pub disable_model_invocation: bool,
}

/// 技能快照（用於傳遞給 Agent）
#[derive(Debug, Clone)]
pub struct SkillSnapshot {
    /// 格式化的 XML 用於系統提示
    pub prompt: String,

    /// 技能列表（含 primaryEnv 用於環境注入）
    pub skills: Vec<SkillSnapshotEntry>,
}

#[derive(Debug, Clone)]
pub struct SkillSnapshotEntry {
    pub name: String,
    pub path: PathBuf,
    pub primary_env: Option<String>,
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
use serde::Serialize;
use std::path::PathBuf;
use thiserror::Error;

/// 技能錯誤 - 必須包含路徑和清楚的訊息
#[derive(Error, Debug, Serialize)]
pub enum SkillError {
    #[error("Cannot read skill '{path}': {message}")]
    FileRead { path: PathBuf, message: String },

    #[error("Invalid YAML in '{path}': {message}")]
    YamlParse { path: PathBuf, message: String },

    #[error("Invalid TOML in '{path}': {message}")]
    TomlParse { path: PathBuf, message: String },

    #[error("Missing frontmatter in '{path}'")]
    MissingFrontmatter { path: PathBuf },

    #[error("Invalid skill name '{name}' in '{path}': {reason}")]
    InvalidName { name: String, path: PathBuf, reason: String },

    #[error("Skill '{name}' not found")]
    NotFound { name: String },

    #[error("Skill '{name}' not eligible: {reason}")]
    NotEligible { name: String, reason: String },

    #[error("System skill install failed: {0}")]
    SystemInstall(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl SkillError {
    /// 記錄到 log 並返回可序列化的訊息
    pub fn log_and_serialize(&self) -> String {
        log::error!("{}", self);
        self.to_string()
    }
}

pub type Result<T> = std::result::Result<T, SkillError>;
```

---

### Phase 3: 資格過濾

#### 步驟 3.1-3.4：實現資格過濾 (`eligibility.rs`)

```rust
use crate::skills::{
    error::{Result, SkillError},
    model::{SkillMetadata, SkillRequirements},
};
use std::env;
use std::path::Path;

/// 資格檢查上下文
pub struct EligibilityContext<'a> {
    /// 應用配置（用於 config 路徑檢查）
    pub config: Option<&'a toml::Value>,
}

/// 檢查技能是否符合資格
pub fn check_eligibility(
    skill: &SkillMetadata,
    skill_config: Option<&SkillConfig>,
    ctx: &EligibilityContext,
) -> Result<()> {
    // 1. 檢查是否明確禁用
    if let Some(cfg) = skill_config {
        if cfg.enabled == Some(false) {
            return Err(SkillError::NotEligible {
                name: skill.name.clone(),
                reason: "Disabled in config".to_string(),
            });
        }
    }

    let metadata = match &skill.openclaw_metadata {
        Some(m) => m,
        None => return Ok(()), // 沒有元資料，預設通過
    };

    // 2. 總是可用標誌
    if metadata.always {
        return Ok(());
    }

    // 3. 作業系統檢查
    if !metadata.os.is_empty() {
        let current_os = get_current_os();
        if !metadata.os.iter().any(|os| os == current_os) {
            return Err(SkillError::NotEligible {
                name: skill.name.clone(),
                reason: format!("OS '{}' not supported", current_os),
            });
        }
    }

    // 4. 檢查所有必需的二進制檔案
    for bin in &metadata.requires.bins {
        if !has_binary(bin) {
            return Err(SkillError::NotEligible {
                name: skill.name.clone(),
                reason: format!("Required binary '{}' not found", bin),
            });
        }
    }

    // 5. 檢查至少一個二進制檔案
    if !metadata.requires.any_bins.is_empty() {
        let has_any = metadata.requires.any_bins.iter().any(|bin| has_binary(bin));
        if !has_any {
            return Err(SkillError::NotEligible {
                name: skill.name.clone(),
                reason: format!(
                    "None of required binaries found: {:?}",
                    metadata.requires.any_bins
                ),
            });
        }
    }

    // 6. 檢查必需的環境變數
    for env_name in &metadata.requires.env {
        // 先檢查環境變數，再檢查 skill config 中的 env
        let has_env = env::var(env_name).is_ok()
            || skill_config
                .and_then(|c| c.env.as_ref())
                .map(|e| e.contains_key(env_name))
                .unwrap_or(false);

        if !has_env {
            return Err(SkillError::NotEligible {
                name: skill.name.clone(),
                reason: format!("Required env var '{}' not set", env_name),
            });
        }
    }

    // 7. 檢查必需的配置路徑
    for config_path in &metadata.requires.config {
        if !is_config_path_truthy(ctx.config, config_path) {
            return Err(SkillError::NotEligible {
                name: skill.name.clone(),
                reason: format!("Required config '{}' not set", config_path),
            });
        }
    }

    Ok(())
}

/// 過濾符合資格的技能
pub fn filter_eligible_skills(
    skills: Vec<SkillMetadata>,
    skill_configs: &HashMap<String, SkillConfig>,
    ctx: &EligibilityContext,
) -> (Vec<SkillMetadata>, Vec<SkillError>) {
    let mut eligible = Vec::new();
    let mut errors = Vec::new();

    for skill in skills {
        let skill_config = skill_configs.get(&skill.name);
        match check_eligibility(&skill, skill_config, ctx) {
            Ok(()) => eligible.push(skill),
            Err(e) => errors.push(e),
        }
    }

    (eligible, errors)
}

/// 檢查二進制檔案是否存在於 PATH 中
fn has_binary(bin: &str) -> bool {
    which::which(bin).is_ok()
}

/// 獲取當前作業系統標識符
fn get_current_os() -> &'static str {
    #[cfg(target_os = "macos")]
    { "darwin" }
    #[cfg(target_os = "linux")]
    { "linux" }
    #[cfg(target_os = "windows")]
    { "win32" }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    { "unknown" }
}

/// 檢查配置路徑是否為真值
fn is_config_path_truthy(config: Option<&toml::Value>, path: &str) -> bool {
    let config = match config {
        Some(c) => c,
        None => return false,
    };

    let parts: Vec<&str> = path.split('.').collect();
    let mut current = config;

    for part in parts {
        match current.get(part) {
            Some(v) => current = v,
            None => return false,
        }
    }

    // 檢查是否為真值
    match current {
        toml::Value::Boolean(b) => *b,
        toml::Value::String(s) => !s.is_empty(),
        toml::Value::Integer(i) => *i != 0,
        toml::Value::Array(a) => !a.is_empty(),
        toml::Value::Table(t) => !t.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_binary() {
        // 這些二進制檔案應該在大多數系統上存在
        #[cfg(unix)]
        assert!(has_binary("ls"));
        #[cfg(windows)]
        assert!(has_binary("cmd"));

        // 這個應該不存在
        assert!(!has_binary("definitely_not_a_real_binary_12345"));
    }

    #[test]
    fn test_get_current_os() {
        let os = get_current_os();
        assert!(["darwin", "linux", "win32", "unknown"].contains(&os));
    }
}
```

---

### Phase 5: Per-skill 配置

#### 步驟 5.1-5.3：實現配置解析 (`config.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 技能系統配置（在 config.yaml 中的 skills 區段）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillsConfig {
    /// 允許的內建技能（如果設置，只有這些內建技能會被使用）
    #[serde(default, rename = "allowBundled")]
    pub allow_bundled: Option<Vec<String>>,

    /// 載入配置
    #[serde(default)]
    pub load: SkillsLoadConfig,

    /// 個別技能配置
    #[serde(default)]
    pub entries: HashMap<String, SkillConfig>,
}

/// 技能載入配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillsLoadConfig {
    /// 額外的技能目錄
    #[serde(default, rename = "extraDirs")]
    pub extra_dirs: Vec<String>,

    /// 監聽變更
    #[serde(default)]
    pub watch: bool,

    /// 監聽防抖間隔（毫秒）
    #[serde(default = "default_debounce", rename = "watchDebounceMs")]
    pub watch_debounce_ms: u64,
}

fn default_debounce() -> u64 { 500 }

/// 個別技能配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillConfig {
    /// 啟用/禁用此技能
    pub enabled: Option<bool>,

    /// API 金鑰（注入到 primaryEnv）
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,

    /// 額外環境變數
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,

    /// 技能特定配置
    #[serde(default)]
    pub config: Option<toml::Value>,
}

/// 解析技能配置查找鍵
pub fn resolve_skill_key(skill: &SkillMetadata) -> &str {
    skill
        .openclaw_metadata
        .as_ref()
        .and_then(|m| m.skill_key.as_deref())
        .unwrap_or(&skill.name)
}

/// 解析個別技能配置
pub fn resolve_skill_config<'a>(
    skills_config: &'a SkillsConfig,
    skill_key: &str,
) -> Option<&'a SkillConfig> {
    skills_config.entries.get(skill_key)
}

/// config.yaml 範例
///
/// ```yaml
/// skills:
///   allowBundled:
///     - github
///     - weather
///   load:
///     extraDirs:
///       - ~/my-skills
///     watch: true
///   entries:
///     github:
///       enabled: true
///       apiKey: ghp_xxxxx
///     weather:
///       enabled: true
///       apiKey: weather-api-key
///       env:
///         WEATHER_CACHE: /tmp/weather
///     spotify-player:
///       enabled: false  # 禁用此技能
/// ```
```

---

### Phase 6: 環境變數注入

#### 步驟 6.1-6.3：實現環境變數注入 (`env_override.rs`)

```rust
use crate::skills::{
    config::{resolve_skill_config, resolve_skill_key, SkillConfig, SkillsConfig},
    model::SkillMetadata,
};
use std::collections::HashMap;
use std::env;

/// 環境變數覆蓋記錄
struct EnvUpdate {
    key: String,
    previous: Option<String>,
}

/// 環境變數恢復句柄
pub struct EnvRestoreHandle {
    updates: Vec<EnvUpdate>,
}

impl EnvRestoreHandle {
    /// 恢復原始環境變數
    pub fn restore(self) {
        for update in self.updates {
            match update.previous {
                Some(prev) => env::set_var(&update.key, prev),
                None => env::remove_var(&update.key),
            }
        }
    }
}

/// 應用技能環境變數覆蓋
pub fn apply_env_overrides(
    skills: &[SkillMetadata],
    skills_config: &SkillsConfig,
) -> EnvRestoreHandle {
    let mut updates = Vec::new();

    for skill in skills {
        let skill_key = resolve_skill_key(skill);
        let skill_config = match resolve_skill_config(skills_config, skill_key) {
            Some(cfg) => cfg,
            None => continue,
        };

        // 注入自定義環境變數
        if let Some(env_map) = &skill_config.env {
            for (env_key, env_value) in env_map {
                // 不覆蓋已存在的環境變數
                if env::var(env_key).is_ok() {
                    continue;
                }

                updates.push(EnvUpdate {
                    key: env_key.clone(),
                    previous: env::var(env_key).ok(),
                });
                env::set_var(env_key, env_value);
            }
        }

        // 注入 API 金鑰到 primaryEnv
        if let (Some(primary_env), Some(api_key)) = (
            skill
                .openclaw_metadata
                .as_ref()
                .and_then(|m| m.primary_env.as_ref()),
            &skill_config.api_key,
        ) {
            // 不覆蓋已存在的環境變數
            if env::var(primary_env).is_err() {
                updates.push(EnvUpdate {
                    key: primary_env.clone(),
                    previous: env::var(primary_env).ok(),
                });
                env::set_var(primary_env, api_key);
            }
        }
    }

    EnvRestoreHandle { updates }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_and_restore() {
        // 設置測試環境
        let test_key = "TEST_SKILL_ENV_12345";
        env::remove_var(test_key);

        let skills = vec![]; // 空技能列表
        let skills_config = SkillsConfig::default();

        let handle = apply_env_overrides(&skills, &skills_config);

        // 恢復
        handle.restore();

        assert!(env::var(test_key).is_err());
    }
}
```

---

### Phase 8: XML 渲染與系統提示

#### 步驟 8.1-8.3：實現 XML 渲染 (`render.rs`)

```rust
use crate::skills::model::{SkillMetadata, SkillSnapshot, SkillSnapshotEntry};

/// 渲染技能列表為 XML 格式
pub fn render_skills_xml(skills: &[SkillMetadata]) -> String {
    if skills.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    output.push_str("<available_skills>\n");

    for skill in skills {
        output.push_str("<skill>\n");
        output.push_str(&format!("<name>{}</name>\n", escape_xml(&skill.name)));
        output.push_str(&format!(
            "<description>{}</description>\n",
            escape_xml(&skill.description)
        ));
        output.push_str(&format!(
            "<location>{}</location>\n",
            skill.path.display()
        ));
        output.push_str("</skill>\n");
    }

    output.push_str("</available_skills>");
    output
}

/// 構建技能系統提示區段
pub fn build_skills_system_prompt(skills_xml: &str, read_tool_name: &str) -> String {
    if skills_xml.is_empty() {
        return String::new();
    }

    format!(
        r#"## Skills (mandatory)
Before replying: scan <available_skills> <description> entries.
- If exactly one skill clearly applies: read its SKILL.md at <location> with `{read_tool}`, then follow it.
- If multiple could apply: choose the most specific one, then read/follow it.
- If none clearly apply: do not read any SKILL.md.
Constraints: never read more than one skill up front; only read after selecting.

{xml}
"#,
        read_tool = read_tool_name,
        xml = skills_xml
    )
}

/// 構建技能快照
pub fn build_skill_snapshot(
    skills: &[SkillMetadata],
    version: u64,
    read_tool_name: &str,
) -> SkillSnapshot {
    // 過濾掉 disable_model_invocation 的技能
    let visible_skills: Vec<_> = skills
        .iter()
        .filter(|s| !s.invocation.disable_model_invocation)
        .collect();

    let skills_xml = render_skills_xml(
        &visible_skills.iter().map(|s| (*s).clone()).collect::<Vec<_>>(),
    );

    let prompt = build_skills_system_prompt(&skills_xml, read_tool_name);

    let entries = skills
        .iter()
        .map(|s| SkillSnapshotEntry {
            name: s.name.clone(),
            path: s.path.clone(),
            primary_env: s
                .openclaw_metadata
                .as_ref()
                .and_then(|m| m.primary_env.clone()),
        })
        .collect();

    SkillSnapshot {
        prompt,
        skills: entries,
        version,
    }
}

/// 轉義 XML 特殊字元
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::model::{SkillInvocation, SkillScope};
    use std::path::PathBuf;

    #[test]
    fn test_render_skills_xml() {
        let skills = vec![SkillMetadata {
            name: "github".to_string(),
            description: "Interact with GitHub using the `gh` CLI.".to_string(),
            path: PathBuf::from("/path/to/github/SKILL.md"),
            scope: SkillScope::User,
            invocation: SkillInvocation::default(),
            openclaw_metadata: None,
            interface: None,
        }];

        let xml = render_skills_xml(&skills);

        assert!(xml.contains("<available_skills>"));
        assert!(xml.contains("<name>github</name>"));
        assert!(xml.contains("Interact with GitHub"));
        assert!(xml.contains("<location>/path/to/github/SKILL.md</location>"));
    }

    #[test]
    fn test_build_skills_system_prompt() {
        let xml = "<available_skills><skill><name>test</name></skill></available_skills>";
        let prompt = build_skills_system_prompt(xml, "Read");

        assert!(prompt.contains("## Skills (mandatory)"));
        assert!(prompt.contains("with `Read`"));
        assert!(prompt.contains(xml));
    }
}
```

---

### Phase 9: 模組匯出

#### `mod.rs`

```rust
pub mod config;
pub mod eligibility;
pub mod env_override;
pub mod error;
pub mod loader;
pub mod manager;
pub mod model;
pub mod render;
pub mod system;

pub use config::{SkillConfig, SkillsConfig, SkillsLoadConfig};
pub use eligibility::{check_eligibility, filter_eligible_skills, EligibilityContext};
pub use env_override::{apply_env_overrides, EnvRestoreHandle};
pub use error::{Result, SkillError};
pub use loader::{load_skills_from_roots, skill_roots_for_cwd};
pub use manager::SkillsManager;
pub use model::{SkillLoadOutcome, SkillMetadata, SkillScope, SkillSnapshot};
pub use render::{build_skill_snapshot, build_skills_system_prompt, render_skills_xml};
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
metadata: '{"requires":{"bins":["git"]}}'
---

# Test Skill Body
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_content).unwrap();

    // 3. 初始化管理器
    let manager = aaagent::skills::SkillsManager::new(app_home);

    // 4. 載入技能
    let snapshot = manager.build_snapshot(&repo);

    // 5. 驗證結果
    assert!(snapshot.prompt.contains("test-skill"));
}
```

---

## 依賴項配置

在 `Cargo.toml` 中添加：

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1.0"  # 用於解析 metadata 中的 JSON
toml = "0.8"
tokio = { version = "1.0", features = ["fs", "rt-multi-thread"] }
walkdir = "2.4"
thiserror = "1.0"
dirs = "5.0"
which = "6.0"  # 用於二進制檔案檢測
log = "0.4"    # 用於錯誤日誌（專案已有此依賴）

[dev-dependencies]
tempfile = "3.8"
pretty_assertions = "1.4"
```

---

## 檢查清單

### 實施前

- [x] 確認 Rust 版本 >= 1.70
- [x] 了解專案整體架構
- [x] 規劃技能目錄結構

### Phase 1: 基礎設施 ✅

- [x] 創建模組結構
- [x] 實現 `model.rs`（含 SkillRequirements, SkillInvocation）
- [x] 實現 `error.rs`
- [x] 編寫單元測試

### Phase 2: 載入器 ✅

- [x] 實現目錄遍歷
- [x] 實現 YAML 解析
- [x] 實現 metadata JSON5 解析
- [x] 實現 TOML 解析 (改用 YAML metadata)
- [x] 實現優先級邏輯
- [x] 編寫整合測試

### Phase 3: 資格過濾 ✅

- [x] 實現二進制檔案檢測
- [x] 實現環境變數檢查
- [x] 實現配置路徑檢查 (在 config.rs 中)
- [x] 實現作業系統過濾
- [x] 編寫過濾測試

### Phase 4: 管理器與快照 ✅

- [x] 實現 `SkillsManager`
- [x] 實現 `snapshot()` 方法
- [x] 測試快照效能

### Phase 5: Per-skill 配置 ✅

- [x] 定義 config.yaml 格式 (SkillsConfig, SkillConfig)
- [x] 實現配置解析
- [x] 實現 enabled/disabled 過濾
- [x] 編寫配置測試

### Phase 6: 環境變數注入 ✅

- [x] 實現 `apply_env_overrides()`
- [x] 實現 `EnvRestoreGuard` (自動恢復)
- [x] 實現 primaryEnv → apiKey 映射
- [x] 編寫注入測試

### Phase 7: 系統技能 (未來擴展)

> **狀態**: 標記為未來擴展，目前技能由用戶手動放置

- [ ] 設計系統技能格式
- [ ] 實現安裝邏輯
- [ ] 實現 fingerprinting
- [ ] 創建範例技能

### Phase 8: XML 渲染與系統提示 ✅

- [x] 實現 `render_skills_xml()` (在 manager.rs 中)
- [x] 實現系統提示區段
- [x] 實現調用控制過濾 (model_invocable, user_invocable)
- [x] 編寫端到端測試

### Phase 9: 整合與優化 ✅

- [x] 整合到 Agent (set_skills_prompt)
- [x] 提供 Web API（/api/skills endpoint）
- [x] 效能優化
- [x] 撰寫文檔 (本文件)
- [x] 完整測試 (24 tests passing)

### 錯誤處理（貫穿所有 Phase） ✅

- [x] 錯誤訊息簡短清楚，包含路徑和上下文
- [x] 錯誤同時輸出到 log 和 API response

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

6. **Plugin 支援**
   - Plugin manifest 註冊
   - Plugin 技能目錄

---

## 參考資源

- [OpenClaw Skill Implementation](https://github.com/openclaw/openclaw) - TypeScript 參考實現
- [Codex CLI Source Code](https://github.com/codex-cli/codex)
- [Serde Documentation](https://serde.rs/)
- [Tokio Documentation](https://tokio.rs/)
- [WalkDir Crate](https://docs.rs/walkdir/)
- [Which Crate](https://docs.rs/which/) - 二進制檔案檢測

---

## 變更紀錄

| 日期 | 版本 | 變更內容 |
|------|------|----------|
| 2026-01-16 | 1.0 | 初始版本 |
| 2026-01-31 | 2.0 | 整合 OpenClaw 改進：資格過濾、調用控制、Per-skill 配置、環境變數注入、使用 Read 工具 |
| 2026-01-31 | 2.1 | 實作完成：Phase 1-6, 8-9 已完成；Phase 7 (系統技能) 標記為未來擴展 |

---

**版本**: 2.1
**最後更新**: 2026-01-31
**狀態**: ✅ 已完成並歸檔
**作者**: Claude Code Implementation Plan Generator
