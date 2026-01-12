# Chat UI AI Error Handling Plan

- Feature name: `chat-ui-error-handling`
- Status: Draft
- Created: 2026-01-06
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)

## 1) Overview

### Goal
Transform raw error messages into user-friendly explanations with actionable suggestions using AI analysis.

### Scope (In)
- Error analysis agent using quick provider
- Comprehensive error context capture
- AI-generated explanations and suggestions
- Severity-based UI styling
- Action buttons (Retry, Edit Input, View Billing)
- Sensitive data sanitization

### Error Types Covered
- Tool execution errors
- API call failures
- LLM provider errors
- Network errors
- Validation errors
- System errors

## 2) Architecture

### Flow

```
Error Occurs
    ↓
Capture error context (type, component, details)
    ↓
Sanitize sensitive data (API keys, tokens)
    ↓
Send to Error Analysis Agent (quick provider)
    ↓
Agent generates:
  - Plain English explanation
  - 2-3 prioritized suggestions
  - Retry guidance
  - Severity level
    ↓
Display Enhanced Error Card in UI
```

## 3) Backend Implementation

### Error Context

```rust
pub struct ErrorContext {
    error_type: ErrorType,        // ToolExecution, ApiCall, LlmProvider, etc.
    component: String,             // Tool name, API endpoint, provider
    raw_error: String,             // Original error message
    details: ErrorDetails {
        tool_arguments: Option<Value>,
        http_status: Option<u16>,
        endpoint: Option<String>,
        model_name: Option<String>,
        token_count: Option<u32>,
        recent_successful_operations: Vec<String>,
        operation_count: usize,
        user_intent: String,
    },
}

pub enum ErrorType {
    ToolExecution,
    ApiCall,
    LlmProvider,
    Network,
    Validation,
    System,
}

pub enum ErrorSeverity {
    Low,      // User can fix or ignore
    Medium,   // Requires action, not urgent
    High,     // Blocks progress, immediate attention
}

pub enum ErrorCategory {
    Transient,      // Temporary, retry likely works
    Configuration,  // Setup issue
    InvalidInput,   // User input problem
    Quota,          // Rate limit or usage limit
    Permission,     // Access denied
    System,         // Internal error
}
```

### Error Analyzer

```rust
pub struct ErrorAnalyzer {
    quick_provider: Box<dyn LLMProvider>,
}

impl ErrorAnalyzer {
    pub async fn analyze_error(
        &self,
        context: ErrorContext,
    ) -> Result<ErrorAnalysis> {
        // Sanitize sensitive data first
        let sanitized_error = sanitize_error(&context.raw_error);
        
        // Build analysis prompt
        let prompt = build_analysis_prompt(&context, &sanitized_error);
        
        // Call quick provider
        let response = self.quick_provider.complete(&prompt).await?;
        
        // Parse JSON response
        let analysis: ErrorAnalysis = serde_json::from_str(&response)?;
        
        Ok(analysis)
    }
}

pub struct ErrorAnalysis {
    explanation: String,           // Plain English (1-2 sentences)
    suggestions: Vec<String>,      // 2-3 specific actions
    is_retryable: bool,
    retry_hint: Option<String>,    // e.g., "Wait 1 minute"
    severity: ErrorSeverity,
    category: ErrorCategory,
}

fn sanitize_error(error: &str) -> String {
    let patterns = [
        r"(api[_-]?key[:\s=]+)[\w-]+",
        r"(token[:\s=]+)[\w-]+",
        r"(bearer\s+)[\w-]+",
        r"(password[:\s=]+)[\w-]+",
    ];
    
    let mut sanitized = error.to_string();
    for pattern in patterns {
        let re = regex::Regex::new(pattern).unwrap();
        sanitized = re.replace_all(&sanitized, "$1***REDACTED***").to_string();
    }
    sanitized
}
```

### Analysis Prompt Template

See full prompt in parent plan (chat-ui-plan.md, lines 646-707).

Key sections:
- Error context with component, type, user intent
- Response format (JSON with explanation, suggestions, severity, category)
- Guidelines (user-friendly, specific, honest, helpful)
- Examples for different error types

### Enhanced AgentEvent

```rust
pub enum AgentEvent {
    // ... existing events ...
    
    ErrorAnalyzed {
        error_id: String,
        error_type: ErrorType,
        component: String,
        raw_error: String,
        analysis: ErrorAnalysis {
            explanation: String,
            suggestions: Vec<String>,
            is_retryable: bool,
            retry_hint: Option<String>,
            severity: ErrorSeverity,
            category: ErrorCategory,
        },
    },
}
```

## 4) Frontend Implementation

### Error Card Component

```typescript
interface ErrorCardProps {
  errorId: string;
  errorType: ErrorType;
  component: string;
  rawError: string;
  analysis: ErrorAnalysis;
  onRetry?: () => void;
  onEditInput?: () => void;
}

function ErrorCard({ component, analysis, rawError, onRetry }: ErrorCardProps) {
  const severityStyles = {
    low: { bg: 'bg-yellow-50', border: 'border-yellow-500', text: 'text-yellow-900', icon: '⚠️' },
    medium: { bg: 'bg-orange-50', border: 'border-orange-500', text: 'text-orange-900', icon: '⚠️' },
    high: { bg: 'bg-red-50', border: 'border-red-500', text: 'text-red-900', icon: '❌' }
  };
  
  const style = severityStyles[analysis.severity];
  
  return (
    <div className={`error-card border-l-4 ${style.border} ${style.bg} p-4 rounded`}>
      {/* Header */}
      <div className="flex items-start gap-3">
        <span className="text-2xl">{style.icon}</span>
        <div className="flex-1">
          <div className="flex items-center gap-2">
            <h4 className={`font-semibold ${style.text}`}>{component}</h4>
            <span className="text-xs px-2 py-0.5 rounded bg-gray-200 text-gray-700">
              {analysis.category}
            </span>
          </div>
          
          {/* AI explanation */}
          <p className={`mt-2 ${style.text}`}>{analysis.explanation}</p>
        </div>
      </div>

      {/* AI suggestions */}
      <div className="mt-4 ml-11">
        <p className={`text-sm font-medium ${style.text} mb-2`}>💡 What you can do:</p>
        <ol className="space-y-2">
          {analysis.suggestions.map((suggestion, i) => (
            <li key={i} className={`text-sm ${style.text} flex gap-2`}>
              <span className="font-semibold">{i + 1}.</span>
              <span>{suggestion}</span>
            </li>
          ))}
        </ol>
      </div>

      {/* Action buttons */}
      {analysis.is_retryable && (
        <div className="mt-4 ml-11 flex gap-2">
          <button onClick={onRetry} className="btn btn-sm btn-primary">
            🔄 Retry {analysis.retry_hint && `(${analysis.retry_hint})`}
          </button>
          {analysis.category === 'invalid_input' && (
            <button className="btn btn-sm btn-secondary">
              ✏️ Edit Input
            </button>
          )}
          {analysis.category === 'quota' && (
            <button className="btn btn-sm btn-accent">
              💳 View Billing
            </button>
          )}
        </div>
      )}

      {/* Technical details (collapsible) */}
      <details className="mt-4 ml-11">
        <summary className="text-xs text-gray-600 cursor-pointer hover:underline">
          Technical details
        </summary>
        <pre className="mt-2 text-xs text-gray-900 bg-gray-100 p-2 rounded overflow-x-auto">
          {rawError}
        </pre>
      </details>
    </div>
  );
}
```

## 5) SSE Integration

### Event Type

```json
{
  "event": "error_analyzed",
  "data": {
    "error_id": "err_xyz789",
    "error_type": "ApiCall",
    "component": "fetch_url",
    "raw_error": "HTTP 429: Rate limit exceeded",
    "analysis": {
      "explanation": "You've made too many requests to this API. The service is asking you to slow down.",
      "suggestions": [
        "Wait 60 seconds before trying again",
        "If you need frequent updates, consider using a webhook instead",
        "Check if you have a rate limit increase available in your API settings"
      ],
      "is_retryable": true,
      "retry_hint": "Wait 1 minute",
      "severity": "medium",
      "category": "quota"
    }
  }
}
```

### Frontend Handler

```typescript
eventSource.addEventListener('error_analyzed', (e) => {
  const data = JSON.parse(e.data);
  const store = useChatStore.getState();
  store.addErrorCard(data);
});
```

## 6) Edge Cases & Fallbacks

**Analysis Failure:**
- Display raw error card if analyzer fails
- Log meta-error for debugging
- Show "Error details unavailable"

**Analysis Timeout:**
- Show raw error immediately (don't block UI)
- Stream in analysis when ready (update card)
- 5s timeout, keep raw error visible

**Cost Control:**
- Only use quick provider (cheap model)
- Cache common error patterns
- Rate limit: max 10 analyses/minute/session

## 7) Testing Plan

- [ ] Error analysis generates valid JSON
- [ ] Sensitive data sanitized
- [ ] All error types handled
- [ ] Severity styling correct
- [ ] Action buttons render based on category
- [ ] Fallback to raw error on analysis failure
- [ ] Cache reduces duplicate analyses

## 8) Acceptance Criteria

- [ ] All errors trigger analysis
- [ ] Plain English explanations displayed
- [ ] 2-3 specific suggestions shown
- [ ] Severity-based styling (yellow/orange/red)
- [ ] Category badges displayed
- [ ] Action buttons work (Retry, Edit, Billing)
- [ ] Technical details collapsible
- [ ] Sensitive data sanitized
- [ ] Graceful degradation on analysis failure

## 9) Implementation Tasks

**Backend:**
- [ ] Create `ErrorAnalyzer` struct
- [ ] Implement analysis prompt builder
- [ ] Add sanitization function
- [ ] Add `AgentEvent::ErrorAnalyzed`
- [ ] Integrate into agent error handling
- [ ] Implement error analysis caching

**Frontend:**
- [ ] Create `ErrorCard` component
- [ ] Add severity styling
- [ ] Implement action buttons
- [ ] Add SSE handler for `error_analyzed`
- [ ] Integrate with Zustand store

---

## References
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)
- Related: [chat-ui-sse-streaming.md](./chat-ui-sse-streaming.md)
