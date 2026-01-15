// Helper functions to convert MessageData to TreeNode format

import type { MessageData, Node } from "../../types/backend";
import type { TreeNode } from "./treeLayout";
import { Role } from "../../types/backend";

/**
 * Convert MessageData to TreeNode format for tree visualization
 */
export function messageToTreeNode(message: MessageData, seq: number): TreeNode {
  // Map Role enum to tree role type
  let role: "user" | "assistant" | "system" | "tool";

  switch (message.role) {
    case Role.User:
      role = "user";
      break;
    case Role.Assistant:
      role = "assistant";
      break;
    case Role.System:
      role = "system";
      break;
    case Role.Tool:
      role = "tool";
      break;
    default:
      role = "system";
  }

  // Determine status
  let status: "loading" | "error" | "complete" | undefined;
  if (message.isStreaming) {
    status = "loading";
  } else if (message.is_error) {
    status = "error";
  } else {
    status = "complete";
  }

  return {
    id: message.id,
    parent_id: null, // TODO: Need parent_id from backend
    role,
    content: message.content,
    seq,
    status,
    timestamp: message.timestamp,
    hasCheckpoint: false, // TODO: Add checkpoint detection
  };
}

/**
 * Convert array of MessageData to TreeNode array
 * All messages now directly match backend structure - no filtering needed
 */
export function messagesToTreeNodes(messages: MessageData[]): TreeNode[] {
  const nodes: TreeNode[] = [];
  let prevNodeId: string | null = null;

  for (let i = 0; i < messages.length; i++) {
    const message = messages[i];

    const node = messageToTreeNode(message, nodes.length);
    node.parent_id = prevNodeId;
    prevNodeId = node.id;

    nodes.push(node);
  }

  return nodes;
}

/**
 * Convert Node (backend format) to TreeNode
 */
export function nodeToTreeNode(node: Node): TreeNode {
  let role: "user" | "assistant" | "system" | "tool";

  if (!node.role) {
    role = "system";
  } else {
    switch (node.role) {
      case Role.User:
        role = "user";
        break;
      case Role.Assistant:
        role = "assistant";
        break;
      case Role.System:
        role = "system";
        break;
      case Role.Tool:
        role = "tool";
        break;
      default:
        role = "system";
    }
  }

  return {
    id: node.node_id,
    parent_id: node.parent_id,
    role,
    content: node.content,
    seq: node.seq,
    timestamp: new Date(node.created_at * 1000),
    hasCheckpoint: false, // TODO: Detect checkpoints
    status: "complete",
  };
}

/**
 * Convert array of Nodes to TreeNode array
 */
export function nodesToTreeNodes(nodes: Node[]): TreeNode[] {
  return nodes.map(nodeToTreeNode);
}
