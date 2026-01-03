# chat_loop_with_tools 完整說明

## 概述

`chat_loop_with_tools` 是一個高層級的 helper 函數，處理完整的 LLM 對話迴圈，包括：
- Streaming 內容輸出
- 自動執行 tools
- 多輪 tool calling
- Loop detection（防止重複呼叫）

## 函數簽名

```rust
pub async fn chat_loop_with_tools<P: LLMProvider>(
    provider: &P,
    messages: Vec<Message>,
    tools: Vec<Tool>,
    config: ChatLoopConfig,
) -> Result<ChatLoopResponse, ProviderError>
```

## ChatLoopConfig 設定

| 欄位 | 型別 | 說明 |
|------|------|------|
| `tool_executors` | `HashMap<String, ToolExecutor>` | Tool 執行器（fallback 用） |
| `registry` | `Option<Arc<ToolRegistry>>` | Tool 註冊表（優先使用） |
| `on_content` | `Option<ContentCallback>` | 文字內容 callback |
| `on_thinking` | `Option<ContentCallback>` | 思考過程 callback |
| `on_tool_calls` | `Option<ToolCallCallback>` | Tool 呼叫前 callback |
| `on_tool_results` | `Option<ToolResultCallback>` | Tool 結果 callback |
| `on_loop_detected` | `Option<LoopDetectionCallback>` | Loop 偵測 callback |
| `max_rounds` | `usize` | 最大輪數（預設 10） |
| `loop_detection` | `Option<LoopDetectorConfig>` | Loop 偵測設定 |

## ChatLoopResponse 回傳值

```rust
pub struct ChatLoopResponse {
    pub content: String,           // 最終內容
    pub usage: TokenUsage,         // Token 使用量
    pub all_tool_calls: Vec<ToolCall>,  // 所有執行過的 tool calls
    pub rounds: usize,             // 執行輪數
}
```

## 完整流程

```
┌─────────────────────────────────────────────────────────────────────┐
│                        chat_loop_with_tools                         │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 初始化                                                               │
│ • full_content = ""                                                 │
│ • all_tool_calls = []                                               │
│ • loop_detector (如果 config.loop_detection 有設定)                  │
│ • handle = provider.chat_loop(messages, tools)                      │
│ • rounds = 0                                                        │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│ while handle.next().await 有事件                                     │
└─────────────────────────────────────────────────────────────────────┘
          │
          ├──▶ LoopStep::Thinking(thought)
          │         └─▶ 呼叫 config.on_thinking callback (如有)
          │
          ├──▶ LoopStep::Content(text)
          │         ├─▶ 累積到 full_content
          │         └─▶ 呼叫 config.on_content callback (如有)
          │
          ├──▶ LoopStep::ToolCallsRequested { tool_calls, content }
          │         │
          │         ├─▶ rounds += 1
          │         │
          │         ├─▶ 檢查 rounds > max_rounds → 回傳錯誤
          │         │
          │         ├─▶ 累積 content 到 full_content
          │         │
          │         ├─▶ 呼叫 config.on_tool_calls callback (如有)
          │         │
          │         ├─▶ Loop Detection (如啟用)
          │         │       │
          │         │       ├─▶ 對每個 call 執行 detector.check()
          │         │       │
          │         │       ├─▶ 有 callback → 由 callback 決定
          │         │       │
          │         │       └─▶ 無 callback → 依 action 處理:
          │         │             • Continue → 繼續
          │         │             • Warn → 收集 warning 到 HashMap
          │         │             • Terminate → 清除 detector，回傳錯誤
          │         │
          │         ├─▶ 執行 Tools (對每個 call)
          │         │       │
          │         │       ├─▶ 有 registry?
          │         │       │       ├─▶ registry.execute(call) 成功 → 用 result
          │         │       │       ├─▶ 失敗 → fallback 到 tool_executors
          │         │       │       └─▶ 都沒有 → "Tool not registered" 錯誤
          │         │       │
          │         │       └─▶ 無 registry?
          │         │               ├─▶ 有 executor → 執行
          │         │               └─▶ 無 → "Tool not registered" 錯誤
          │         │
          │         ├─▶ 如有 loop warning → prepend 到 result.content
          │         │
          │         ├─▶ 呼叫 config.on_tool_results callback (如有)
          │         │
          │         └─▶ handle.submit_tool_results(results) → 繼續迴圈
          │
          ├──▶ LoopStep::ToolResultsReceived
          │         └─▶ (忽略，繼續迴圈)
          │
          └──▶ LoopStep::Done { content, total_usage, ... }
                    │
                    ├─▶ 更新 full_content (如有新內容)
                    │
                    └─▶ 回傳 ChatLoopResponse {
                            content: full_content,
                            usage: total_usage,
                            all_tool_calls,
                            rounds
                        }
```

## 關鍵特性

| 特性 | 說明 |
|------|------|
| **Tool 查找順序** | Registry → tool_executors → 錯誤 |
| **Loop Detection** | 可選，Warning 會 prepend 到 tool result 而非獨立提交 |
| **Streaming** | 透過 callbacks (on_content, on_thinking, on_tool_calls, on_tool_results) |
| **Max Rounds** | 預設 10，防止無限迴圈 |
| **Registry 不持鎖** | `execute(&self)` 內部 clone Arc，不會跨 await 持有 lock |

---

# LoopStep 事件詳細說明

## 事件類型

```rust
pub enum LoopStep {
    Thinking(String),
    Content(String),
    ToolCallsRequested { tool_calls, content },
    ToolResultsReceived { count },
    Done { content, finish_reason, total_usage, all_tool_calls },
}
```

---

## 1. `LoopStep::Thinking(String)`

**用途**: LLM 的思考/推理過程（例如 Claude 的 extended thinking、OpenAI o1 的 reasoning）

**觸發時機**: Provider streaming 時收到 thinking token

**使用者處理**:
```rust
LoopStep::Thinking(thought) => {
    // 可選：顯示思考過程給使用者
    println!("[Thinking] {}", thought);
}
```

**注意**: 不是所有 provider/model 都支援

---

## 2. `LoopStep::Content(String)`

**用途**: LLM 生成的文字內容（streaming delta）

**觸發時機**: Provider 收到每個文字 chunk

**產生方式** (OpenAI):
```rust
// openai.rs
if let Some(content) = choice.delta.content {
    content_accumulator.push_str(&content);
    event_tx.send(Ok(LoopStep::Content(content)));
}
```

**使用者處理**:
```rust
LoopStep::Content(text) => {
    // Streaming 輸出到終端
    print!("{}", text);
    stdout().flush();
}
```

**注意**: 多個 Content 事件會累積成完整回應

---

## 3. `LoopStep::ToolCallsRequested { tool_calls, content }`

**用途**: LLM 請求執行一個或多個 tools

**欄位**:
| 欄位 | 型別 | 說明 |
|------|------|------|
| `tool_calls` | `Vec<ToolCall>` | 要執行的 tool calls (可能多個 parallel) |
| `content` | `String` | Tool call 前的文字內容（可能為空） |

**ToolCall 結構**:
```rust
pub struct ToolCall {
    pub id: String,           // Tool call ID (用於匹配 result)
    pub name: String,         // Tool 名稱
    pub arguments: Value,     // JSON 參數
}
```

**觸發時機**: LLM 回應的 `finish_reason` 是 `tool_calls` 時

**產生方式** (OpenAI):
```rust
// openai.rs
if !tool_call_assembler.is_empty() {
    let tool_calls = tool_call_assembler.drain().collect();
    completed_tool_calls = Some(tool_calls.clone());
    event_tx.send(Ok(LoopStep::ToolCallsRequested {
        tool_calls,
        content: content_accumulator.clone(),
    }));
}
```

**使用者處理**:
```rust
LoopStep::ToolCallsRequested { tool_calls, content } => {
    // 1. 執行每個 tool
    let mut results = Vec::new();
    for call in &tool_calls {
        let result = execute_tool(call).await;
        results.push(result);
    }
    
    // 2. 提交結果，讓 LLM 繼續
    handle.submit_tool_results(results)?;
}
```

**關鍵**: 必須呼叫 `submit_tool_results()` 否則迴圈會卡住

---

## 4. `LoopStep::ToolResultsReceived { count }`

**用途**: 確認 tool results 已被接收並處理

**欄位**:
| 欄位 | 型別 | 說明 |
|------|------|------|
| `count` | `usize` | 收到的 tool results 數量 |

**觸發時機**: `submit_tool_results()` 被呼叫後，背景任務處理完畢

**產生方式** (OpenAI):
```rust
// openai.rs
ChatLoopCommand::SubmitToolResults(results) => {
    let result_count = results.len();
    event_tx.send(Ok(LoopStep::ToolResultsReceived {
        count: result_count,
    }));
    // ... 繼續發送下一輪請求
}
```

**使用者處理**:
```rust
LoopStep::ToolResultsReceived { count } => {
    // 可選：UI 回饋
    println!("Submitted {} tool results", count);
    // 通常忽略，繼續等待下一個事件
}
```

**注意**: 這是 optional 的 UI 回饋事件，`chat_loop_with_tools` 直接忽略它

---

## 5. `LoopStep::Done { content, finish_reason, total_usage, all_tool_calls }`

**用途**: 對話完成

**欄位**:
| 欄位 | 型別 | 說明 |
|------|------|------|
| `content` | `String` | 最終完整回應 |
| `finish_reason` | `FinishReason` | 結束原因 (Stop, Length, ToolCalls, Error) |
| `total_usage` | `TokenUsage` | 整個迴圈的 token 使用量 |
| `all_tool_calls` | `Vec<ToolCall>` | 所有執行過的 tool calls |

**觸發時機**: LLM 回應 `finish_reason: stop` 且沒有 tool calls

**產生方式** (OpenAI):
```rust
// openai.rs
} else {
    // Text response completed
    event_tx.send(Ok(LoopStep::Done {
        content: content_accumulator.clone(),
        finish_reason: FinishReason::Stop,
        total_usage: token_usage,
        all_tool_calls: all_tool_calls_in_loop.clone(),
    }));
}
```

**使用者處理**:
```rust
LoopStep::Done { content, total_usage, .. } => {
    println!("\n\nDone! Used {} tokens", total_usage.total());
    return Ok(content);
}
```

---

## 事件流程圖

```
┌────────────────────────────────────────────────────────────────────────────┐
│                           典型對話流程                                      │
└────────────────────────────────────────────────────────────────────────────┘

User: "列出當前目錄的檔案"
         │
         ▼
    ┌─────────┐
    │ LLM API │
    └────┬────┘
         │
         ▼
    ┌─────────────────────────────┐
    │ Content("我來幫你")          │  ← Streaming 文字
    │ Content("執行")              │
    │ Content("ls 指令")           │
    └─────────────────────────────┘
         │
         ▼
    ┌─────────────────────────────┐
    │ ToolCallsRequested {        │  ← LLM 請求 tool
    │   tool_calls: [             │
    │     { name: "bash",         │
    │       arguments: {          │
    │         "command": "ls"     │
    │       }                     │
    │     }                       │
    │   ],                        │
    │   content: "我來幫你執行..." │
    │ }                           │
    └─────────────────────────────┘
         │
         ▼
    ┌─────────────────────────────┐
    │ 使用者執行 tool              │
    │ submit_tool_results([       │
    │   { content: "file1.txt\n   │
    │              file2.rs" }    │
    │ ])                          │
    └─────────────────────────────┘
         │
         ▼
    ┌─────────────────────────────┐
    │ ToolResultsReceived {       │  ← 確認收到
    │   count: 1                  │
    │ }                           │
    └─────────────────────────────┘
         │
         ▼
    ┌─────────────────────────────┐
    │ Content("目錄中有")          │  ← LLM 處理結果
    │ Content("2個檔案:")          │
    │ Content("\n- file1.txt")    │
    │ Content("\n- file2.rs")     │
    └─────────────────────────────┘
         │
         ▼
    ┌─────────────────────────────┐
    │ Done {                      │  ← 對話結束
    │   content: "目錄中有2個...", │
    │   finish_reason: Stop,      │
    │   total_usage: {...},       │
    │   all_tool_calls: [...]     │
    │ }                           │
    └─────────────────────────────┘
```

---

## 多輪 Tool Call 流程

```
ToolCallsRequested → 執行 → submit_tool_results
         ↓
ToolResultsReceived
         ↓
Content (LLM 分析結果)
         ↓
ToolCallsRequested → 執行 → submit_tool_results  ← 可能需要更多 tools
         ↓
ToolResultsReceived
         ↓
Content
         ↓
Done  ← 最終完成
```

---

## 使用範例

```rust
use km_tools::llm::*;
use km_tools::tools::BashTool;
use std::sync::Arc;

async fn example() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let provider = OpenAIProvider::create("gpt-4".to_string(), api_key)?;
    
    // 方式 1: 使用 Registry
    let registry = Arc::new(ToolRegistry::new().register_all_builtin());
    let tools = registry.get_tools_for_llm();
    
    let config = ChatLoopConfig::new()
        .with_registry(registry)
        .on_content(|text| print!("{}", text))
        .on_tool_calls(|calls| {
            println!("\n[Calling {} tools]", calls.len());
        })
        .with_max_rounds(20);
    
    // 方式 2: 使用 tool_executors (可與 registry 混用)
    let bash_tool = BashTool::new();
    let config = ChatLoopConfig::new()
        .with_tool("bash", {
            let bash_tool = bash_tool.clone();
            move |call| {
                let bash_tool = bash_tool.clone();
                async move { bash_tool.execute(&call).await }
            }
        })
        .on_content(|text| print!("{}", text));
    
    let response = chat_loop_with_tools(
        &provider,
        vec![Message {
            role: Role::User,
            content: "List files in current directory".to_string(),
            tool_call_id: None,
            tool_calls: None,
        }],
        tools,
        config
    ).await?;
    
    println!("\nDone! Used {} tokens in {} rounds", 
             response.usage.total(), 
             response.rounds);
    
    Ok(())
}
```
