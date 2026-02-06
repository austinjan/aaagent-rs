// NodeDetailsPanel - Displays detailed information about a selected tree node

import { useState } from "react";
import { Clock, User, Bot, Wrench, AlertCircle, CheckCircle2, Copy, Check } from "lucide-react";
import { Button } from "../ui/button";
import { MessageToolbar } from "../chat/MessageToolbar";
import type { TreeNode, ToolCall } from "./treeLayout";

export interface NodeDetailsPanelProps {
  node: TreeNode | null;
  onClose?: () => void;

  // Checkpoint and branch operations
  canCreateCheckpoint?: boolean;
  onCreateCheckpoint?: (nodeId: string) => void;
  onBranchAfter?: (nodeId: string) => void;
  onBranchAlternative?: (nodeId: string) => void;
}

export function NodeDetailsPanel({
  node,
  onClose,
  canCreateCheckpoint = false,
  onCreateCheckpoint,
  onBranchAfter,
  onBranchAlternative,
}: NodeDetailsPanelProps) {
  const [copied, setCopied] = useState(false);

  if (!node) {
    return (
      <div className="h-full flex items-center justify-center text-muted-foreground">
        <p>Select a node to view details</p>
      </div>
    );
  }

  const getRoleIcon = () => {
    switch (node.role) {
      case "user":
        return <User className="h-5 w-5" />;
      case "assistant":
        return <Bot className="h-5 w-5" />;
      case "tool":
        return <Wrench className="h-5 w-5" />;
      case "system":
        return <AlertCircle className="h-5 w-5" />;
      default:
        return null;
    }
  };

  const getRoleLabel = () => {
    const labels: Record<string, string> = {
      user: "User",
      assistant: "Assistant",
      tool: "Tool",
      system: "System",
    };
    return labels[node.role] || node.role;
  };

  const getRoleColor = () => {
    const colors: Record<string, string> = {
      user: "text-blue-600 dark:text-blue-400",
      assistant: "text-green-600 dark:text-green-400",
      tool: "text-orange-600 dark:text-orange-400",
      system: "text-gray-600 dark:text-gray-400",
    };
    return colors[node.role] || "text-gray-600";
  };

  const formatTimestamp = () => {
    if (!node.timestamp) return "N/A";
    const date = new Date(node.timestamp);
    return date.toLocaleString();
  };

  const handleCopyContent = async () => {
    try {
      await navigator.clipboard.writeText(node.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  const handleCopyNodeId = async () => {
    try {
      await navigator.clipboard.writeText(node.id);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  return (
    <div className="h-full flex flex-col bg-background border-l border-border">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border">
        <div className="flex items-center gap-2">
          <div className={getRoleColor()}>{getRoleIcon()}</div>
          <h3 className="font-semibold">Node Details</h3>
        </div>
        <div className="flex items-center gap-2">
          <MessageToolbar
            nodeId={node.id}
            role={node.role as "user" | "assistant" | "system" | "tool"}
            canCreateCheckpoint={canCreateCheckpoint}
            onCreateCheckpoint={onCreateCheckpoint}
            onBranchAfter={onBranchAfter}
            onBranchAlternative={onBranchAlternative}
          />
          {onClose && (
            <Button variant="ghost" size="sm" onClick={onClose}>
              ✕
            </Button>
          )}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Role */}
        <div>
          <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
            Role
          </label>
          <div className={`flex items-center gap-2 mt-1 ${getRoleColor()}`}>
            {getRoleIcon()}
            <span className="font-medium">{getRoleLabel()}</span>
          </div>
        </div>

        {/* Node ID */}
        <div>
          <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
            Node ID
          </label>
          <div className="flex items-center gap-2 mt-1">
            <code className="flex-1 text-xs bg-muted px-2 py-1 rounded font-mono">
              {node.id}
            </code>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleCopyNodeId}
              className="h-7 w-7 p-0"
              title="Copy node ID"
            >
              {copied ? (
                <Check className="h-3 w-3 text-green-600" />
              ) : (
                <Copy className="h-3 w-3" />
              )}
            </Button>
          </div>
        </div>

        {/* Timestamp */}
        {node.timestamp && (
          <div>
            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              Timestamp
            </label>
            <div className="flex items-center gap-2 mt-1 text-sm">
              <Clock className="h-4 w-4 text-muted-foreground" />
              <span>{formatTimestamp()}</span>
            </div>
          </div>
        )}

        {/* Status */}
        {node.status && (
          <div>
            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              Status
            </label>
            <div className="flex items-center gap-2 mt-1">
              {node.status === "complete" && (
                <>
                  <CheckCircle2 className="h-4 w-4 text-green-600" />
                  <span className="text-sm text-green-600">Complete</span>
                </>
              )}
              {node.status === "error" && (
                <>
                  <AlertCircle className="h-4 w-4 text-red-600" />
                  <span className="text-sm text-red-600">Error</span>
                </>
              )}
              {node.status === "loading" && (
                <>
                  <div className="h-4 w-4 border-2 border-blue-600 border-t-transparent rounded-full animate-spin" />
                  <span className="text-sm text-blue-600">Loading</span>
                </>
              )}
            </div>
          </div>
        )}

        {/* Sequence Number */}
        <div>
          <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
            Sequence
          </label>
          <div className="mt-1 text-sm">#{node.seq}</div>
        </div>

        {/* Checkpoint Info */}
        {node.hasCheckpoint && (
          <div>
            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              Checkpoint
            </label>
            <div className="mt-1 p-3 bg-amber-50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-800 rounded-md">
              <div className="flex items-center gap-2 text-amber-700 dark:text-amber-400 font-medium mb-2">
                📌 Checkpoint Node
              </div>
              {node.checkpointSummary && (
                <p className="text-sm text-amber-900 dark:text-amber-300">
                  {node.checkpointSummary}
                </p>
              )}
            </div>
          </div>
        )}

        {/* In Context */}
        {node.inContext !== undefined && (
          <div>
            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              In Context
            </label>
            <div className="mt-1 text-sm">
              {node.inContext ? (
                <span className="text-green-600">✓ Yes</span>
              ) : (
                <span className="text-muted-foreground">✗ No</span>
              )}
            </div>
          </div>
        )}

        {/* Tool Call ID (for tool result nodes) */}
        {node.tool_call_id && (
          <div>
            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              Tool Call ID
            </label>
            <div className="mt-1">
              <code className="text-xs bg-muted px-2 py-1 rounded font-mono">
                {node.tool_call_id}
              </code>
            </div>
          </div>
        )}

        {/* Tool Calls (for assistant nodes that invoked tools) */}
        {node.tool_calls && node.tool_calls.length > 0 && (
          <div>
            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              Tool Calls ({node.tool_calls.length})
            </label>
            <div className="mt-2 space-y-3">
              {node.tool_calls.map((toolCall: ToolCall, index: number) => (
                <div
                  key={toolCall.id || index}
                  className="p-3 bg-orange-50 dark:bg-orange-950/20 border border-orange-200 dark:border-orange-800 rounded-md"
                >
                  <div className="flex items-center gap-2 mb-2">
                    <Wrench className="h-4 w-4 text-orange-600 dark:text-orange-400" />
                    <span className="font-medium text-orange-700 dark:text-orange-300">
                      {toolCall.name}
                    </span>
                  </div>
                  <div className="text-xs text-muted-foreground mb-1">
                    ID: <code className="bg-muted px-1 rounded">{toolCall.id}</code>
                  </div>
                  <div className="mt-2">
                    <div className="text-xs text-muted-foreground mb-1">Arguments:</div>
                    <pre className="text-xs bg-muted p-2 rounded overflow-x-auto max-h-[200px] overflow-y-auto">
                      {JSON.stringify(toolCall.arguments, null, 2)}
                    </pre>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Token Usage (for assistant nodes) */}
        {node.role === "assistant" && node.token_usage && (
          <div>
            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              Token Usage
            </label>
            <div className="mt-2 p-3 bg-blue-50 dark:bg-blue-950/20 border border-blue-200 dark:border-blue-800 rounded-md">
              <div className="space-y-2">
                <div className="flex justify-between items-center">
                  <span className="text-sm font-medium text-blue-700 dark:text-blue-300">
                    Total
                  </span>
                  <span className="text-sm font-bold text-blue-900 dark:text-blue-100">
                    {(node.token_usage.input_tokens + node.token_usage.output_tokens).toLocaleString()} tokens
                  </span>
                </div>
                <div className="flex justify-between items-center text-xs">
                  <span className="text-muted-foreground">Input</span>
                  <span>{node.token_usage.input_tokens.toLocaleString()}</span>
                </div>
                <div className="flex justify-between items-center text-xs">
                  <span className="text-muted-foreground">Output</span>
                  <span>{node.token_usage.output_tokens.toLocaleString()}</span>
                </div>
                {node.token_usage.cached_tokens > 0 && (
                  <div className="flex justify-between items-center text-xs">
                    <span className="text-muted-foreground">Cached</span>
                    <span className="text-green-600 dark:text-green-400">
                      {node.token_usage.cached_tokens.toLocaleString()}
                    </span>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Content */}
        <div>
          <div className="flex items-center justify-between mb-1">
            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              Content
            </label>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleCopyContent}
              className="h-7 gap-1 text-xs"
            >
              {copied ? (
                <>
                  <Check className="h-3 w-3" />
                  Copied
                </>
              ) : (
                <>
                  <Copy className="h-3 w-3" />
                  Copy
                </>
              )}
            </Button>
          </div>
          <div className="mt-1 p-3 bg-muted rounded-md border border-border max-h-[400px] overflow-y-auto">
            <pre className="text-sm whitespace-pre-wrap break-words font-sans">
              {node.content}
            </pre>
          </div>
        </div>
      </div>
    </div>
  );
}
