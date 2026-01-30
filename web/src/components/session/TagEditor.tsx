// TagEditor - Component for managing session tags

import { useState } from "react";
import { X, Plus, Check } from "lucide-react";
import { Button } from "../ui/button";
import { Input } from "../ui/input";

export interface TagEditorProps {
  sessionId: string;
  initialTags: string[];
  onTagsUpdate: (tags: string[]) => Promise<void>;
  disabled?: boolean;
}

export function TagEditor({
  sessionId: _sessionId,
  initialTags,
  onTagsUpdate,
  disabled = false,
}: TagEditorProps) {
  const [tags, setTags] = useState<string[]>(initialTags);
  const [inputValue, setInputValue] = useState("");
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  const handleAddTag = () => {
    const trimmed = inputValue.trim().toLowerCase();
    if (trimmed && !tags.includes(trimmed)) {
      setTags([...tags, trimmed]);
      setInputValue("");
    }
  };

  const handleRemoveTag = (tagToRemove: string) => {
    setTags(tags.filter((tag) => tag !== tagToRemove));
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleAddTag();
    } else if (e.key === "Escape") {
      setInputValue("");
      setIsEditing(false);
      setTags(initialTags);
    }
  };

  const handleSave = async () => {
    try {
      setIsSaving(true);
      await onTagsUpdate(tags);
      setIsEditing(false);
    } catch (err) {
      console.error("Failed to update tags:", err);
      alert("Failed to update tags. Please try again.");
      setTags(initialTags);
    } finally {
      setIsSaving(false);
    }
  };

  const handleCancel = () => {
    setTags(initialTags);
    setInputValue("");
    setIsEditing(false);
  };

  const hasChanges = JSON.stringify(tags) !== JSON.stringify(initialTags);

  if (!isEditing) {
    return (
      <div className="flex items-center gap-2">
        {/* Display tags */}
        {tags.length > 0 ? (
          <div className="flex flex-wrap gap-1">
            {tags.map((tag) => (
              <span
                key={tag}
                className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-primary/10 text-primary"
              >
                {tag}
              </span>
            ))}
          </div>
        ) : (
          <span className="text-xs text-muted-foreground">No tags</span>
        )}

        {/* Edit button */}
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setIsEditing(true)}
          disabled={disabled}
          className="h-6 px-2 text-xs"
        >
          Edit Tags
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {/* Current tags */}
      <div className="flex flex-wrap gap-1">
        {tags.map((tag) => (
          <span
            key={tag}
            className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-primary/10 text-primary"
          >
            {tag}
            <button
              onClick={() => handleRemoveTag(tag)}
              className="hover:text-primary/70"
              disabled={disabled || isSaving}
            >
              <X className="h-3 w-3" />
            </button>
          </span>
        ))}
      </div>

      {/* Input for new tag */}
      <div className="flex items-center gap-2">
        <Input
          type="text"
          placeholder="Add tag..."
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={disabled || isSaving}
          className="h-7 text-xs flex-1"
          autoFocus
        />
        <Button
          variant="ghost"
          size="sm"
          onClick={handleAddTag}
          disabled={disabled || isSaving || !inputValue.trim()}
          className="h-7 w-7 p-0"
          title="Add tag"
        >
          <Plus className="h-3 w-3" />
        </Button>
      </div>

      {/* Action buttons */}
      <div className="flex items-center gap-2">
        <Button
          variant="default"
          size="sm"
          onClick={handleSave}
          disabled={disabled || isSaving || !hasChanges}
          className="h-7 text-xs"
        >
          <Check className="h-3 w-3 mr-1" />
          {isSaving ? "Saving..." : "Save"}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={handleCancel}
          disabled={disabled || isSaving}
          className="h-7 text-xs"
        >
          Cancel
        </Button>
      </div>
    </div>
  );
}
