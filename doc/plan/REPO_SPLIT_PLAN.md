# km-tools 獨立 Repo 遷移計畫

## 概述

將 `km-tools` 從 `km` 知識庫分離成獨立的 Git repository。

## 新 Repo 命名建議

| 名稱 | 優點 | 缺點 |
|------|------|------|
| `llm-rs` | 簡潔、好記 | 可能已被佔用 |
| `llm-agent-rs` | 描述性強 | 稍長 |
| `agentic` | 現代感 | 不明確是 Rust |
| `km-tools` | 保持原名 | 與知識庫關聯不清 |
| `llm-kit` | 簡潔、描述性 | 一般 |

**建議**: `llm-kit` 或 `llm-agent-rs`

---

## 遷移步驟

### Phase 1: 準備工作

```bash
# 1. 確保 km repo 狀態乾淨
cd D:/code/km
git status
git stash  # 如果有未提交的變更

# 2. 建立備份分支
git checkout main
git pull
git checkout -b backup/before-split
git push origin backup/before-split
```

### Phase 2: 提取 km-tools 歷史

使用 `git subtree split` 保留完整歷史：

```bash
# 3. 在 km repo 中執行 subtree split
cd D:/code/km
git subtree split -P km-tools -b km-tools-split

# 這會建立一個只包含 km-tools 歷史的分支
```

### Phase 3: 建立新 Repo

```bash
# 4. 在 GitHub 建立新的空 repo (不要加 README/LICENSE)
# 名稱: llm-kit (或你選擇的名稱)

# 5. Clone 並推送
cd D:/code
mkdir llm-kit
cd llm-kit
git init
git pull ../km km-tools-split
git remote add origin git@github.com:YOUR_USERNAME/llm-kit.git
git branch -M main
git push -u origin main
```

### Phase 4: 更新新 Repo

```bash
# 6. 更新 Cargo.toml
cd D:/code/llm-kit
```

更新 `Cargo.toml`:
```toml
[package]
name = "llm-kit"  # 或選擇的名稱
version = "0.1.0"
edition = "2021"
description = "Unified LLM provider abstraction with streaming, tool calling, and agent support"
license = "MIT"
repository = "https://github.com/YOUR_USERNAME/llm-kit"
keywords = ["llm", "openai", "anthropic", "agent", "streaming"]
categories = ["api-bindings", "asynchronous"]

# ... 其餘依賴保持不變
```

```bash
# 7. 新增必要檔案
# - LICENSE (MIT)
# - README.md (專案說明)
# - .github/workflows/ci.yml (CI/CD)

# 8. 移除 km 專屬內容
rm -rf scripts/  # 如果有 km 專屬腳本
# 檢查 doc/ 是否有需要清理的內容
```

### Phase 5: 清理 km Repo

```bash
# 9. 從 km 移除 km-tools
cd D:/code/km
git checkout main
rm -rf km-tools/
rm km-tools.exe  # 移除編譯產物
git add -A
git commit -m "chore: Extract km-tools to separate repository"

# 10. 更新 km 的 README 或 build.py 
# 指向新的 llm-kit repo
```

### Phase 6: 設定 CI/CD (可選)

建立 `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --verbose
      - run: cargo test --verbose
      - run: cargo clippy -- -D warnings

  fmt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check
```

---

## 新 Repo 結構

```
llm-kit/
├── .github/
│   └── workflows/
│       └── ci.yml
├── src/
│   ├── lib.rs
│   ├── llm/
│   │   ├── mod.rs
│   │   ├── provider.rs
│   │   ├── openai.rs
│   │   ├── anthropic.rs
│   │   ├── gemini.rs
│   │   ├── helpers.rs
│   │   ├── registry.rs
│   │   └── loop_detector.rs
│   └── tools/
│       ├── mod.rs
│       ├── bash.rs
│       └── editor_edit.rs
├── examples/
│   ├── openai_basic.rs
│   ├── simple_agent.rs
│   └── interactive_agent.rs
├── Cargo.toml
├── README.md
├── LICENSE
└── CHANGELOG.md
```

---

## 決策點

### 1. main.rs 怎麼處理？

目前 `km-tools` 有 CLI (`main.rs`)，選項：

- **A) 移除 CLI**: 只保留 library，CLI 放在 km 或另一個 repo
- **B) 保留 CLI**: 作為範例/開發工具
- **C) 分離 binary**: `llm-kit` (lib) + `llm-kit-cli` (bin)

**建議**: B - 保留但簡化，主要價值在 library

### 2. doc/plan 怎麼處理？

- 已完成的設計文件移到 `docs/design/` (參考用)
- 刪除過時的計畫文件
- `LLM_IMPLEMENTATION_STATUS.md` 可以變成 `CHANGELOG.md`

### 3. 是否立即發布到 crates.io？

**建議**: 先不發布，等：
- API 穩定
- 完成 Gemini provider
- 寫好文檔和範例

---

## 執行檢查清單

- [ ] 備份 km repo (backup/before-split 分支)
- [ ] 執行 git subtree split
- [ ] 在 GitHub 建立新 repo
- [ ] 推送歷史到新 repo
- [ ] 更新 Cargo.toml (名稱、metadata)
- [ ] 新增 LICENSE
- [ ] 新增 README.md
- [ ] 清理 km 專屬內容
- [ ] 設定 CI/CD
- [ ] 從 km 移除 km-tools
- [ ] 更新 km 的參照
- [ ] 測試新 repo 可以編譯和測試
