import { useState, useMemo } from "react";
import { Sparkles, Search, AlertCircle, X, ChevronDown, RefreshCw } from "lucide-react";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { cn } from "@/lib/utils";
import type { Skill, SkillError } from "../../types/backend";

export interface SkillPickerProps {
  skills: Skill[];
  errors: SkillError[];
  loading: boolean;
  selectedSkill: Skill | null;
  onSelectSkill: (skill: Skill | null) => void;
  onRefresh?: () => void;
  disabled?: boolean;
}

export function SkillPicker({
  skills,
  errors,
  loading,
  selectedSkill,
  onSelectSkill,
  onRefresh,
  disabled = false,
}: SkillPickerProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [showErrors, setShowErrors] = useState(false);

  const filteredSkills = useMemo(() => {
    if (!searchQuery.trim()) return skills;
    const query = searchQuery.toLowerCase();
    return skills.filter(
      (skill) =>
        skill.name.toLowerCase().includes(query) ||
        skill.description.toLowerCase().includes(query),
    );
  }, [skills, searchQuery]);

  const handleSelectSkill = (skill: Skill) => {
    onSelectSkill(skill);
    setIsOpen(false);
    setSearchQuery("");
  };

  const errorCount = errors.length;

  return (
    <div className="relative">
      {/* Skills button */}
      <Button
        variant={selectedSkill ? "default" : "outline"}
        size="sm"
        onClick={() => setIsOpen(!isOpen)}
        disabled={disabled || loading}
        className={cn(
          "gap-2 transition-all",
          selectedSkill && "bg-primary text-primary-foreground",
        )}
      >
        <Sparkles className="h-4 w-4" />
        <span>Skills</span>
        {skills.length > 0 && !selectedSkill && (
          <span className="text-xs opacity-70">({skills.length})</span>
        )}
        {errorCount > 0 && (
          <span className="ml-1 flex h-4 w-4 items-center justify-center rounded-full bg-destructive text-[10px] text-destructive-foreground">
            {errorCount}
          </span>
        )}
        <ChevronDown
          className={cn(
            "h-3 w-3 transition-transform",
            isOpen && "rotate-180",
          )}
        />
      </Button>

      {/* Popover */}
      {isOpen && (
        <>
          {/* Backdrop */}
          <div
            className="fixed inset-0 z-40"
            onClick={() => {
              setIsOpen(false);
              setSearchQuery("");
              setShowErrors(false);
            }}
          />

          {/* Dropdown panel */}
          <div className="absolute bottom-full left-0 mb-2 z-50 w-80 rounded-lg border border-border bg-popover shadow-lg">
            {/* Header */}
            <div className="flex items-center justify-between border-b border-border px-3 py-2">
              <span className="text-sm font-medium">Available Skills</span>
              <div className="flex items-center gap-1">
                {onRefresh && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => onRefresh()}
                    disabled={loading}
                    className="h-6 w-6 p-0"
                    title="Refresh skills"
                  >
                    <RefreshCw className={cn("h-3 w-3", loading && "animate-spin")} />
                  </Button>
                )}
                {errorCount > 0 && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setShowErrors(!showErrors)}
                    className="h-6 gap-1 px-2 text-xs text-destructive hover:text-destructive"
                  >
                    <AlertCircle className="h-3 w-3" />
                    {errorCount} {errorCount === 1 ? "Error" : "Errors"}
                  </Button>
                )}
              </div>
            </div>

            {showErrors ? (
              /* Error list */
              <div className="max-h-64 overflow-y-auto p-2">
                <div className="space-y-2">
                  {errors.map((err, idx) => (
                    <div
                      key={idx}
                      className="rounded-md bg-destructive/10 p-2 text-xs"
                    >
                      <div className="font-medium text-destructive">
                        {err.category}
                      </div>
                      <div className="mt-1 text-muted-foreground">
                        {err.error}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <>
                {/* Search input */}
                <div className="border-b border-border p-2">
                  <div className="relative">
                    <Search className="absolute left-2 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                    <Input
                      type="text"
                      placeholder="Search skills..."
                      value={searchQuery}
                      onChange={(e) => setSearchQuery(e.target.value)}
                      className="h-8 pl-8 text-sm"
                      autoFocus
                    />
                  </div>
                </div>

                {/* Skills list */}
                <div className="max-h-64 overflow-y-auto p-1">
                  {loading ? (
                    <div className="flex items-center justify-center py-8 text-sm text-muted-foreground">
                      Loading skills...
                    </div>
                  ) : filteredSkills.length === 0 ? (
                    <div className="flex items-center justify-center py-8 text-sm text-muted-foreground">
                      {searchQuery
                        ? "No skills match your search"
                        : "No skills available"}
                    </div>
                  ) : (
                    <div className="space-y-0.5">
                      {filteredSkills.map((skill) => (
                        <button
                          key={skill.name}
                          onClick={() => handleSelectSkill(skill)}
                          className={cn(
                            "flex w-full flex-col items-start gap-0.5 rounded-md px-3 py-2 text-left transition-colors",
                            "hover:bg-accent",
                            selectedSkill?.name === skill.name &&
                              "bg-accent/50",
                          )}
                        >
                          <div className="flex items-center gap-2">
                            <span
                              className={cn(
                                "h-1.5 w-1.5 rounded-full",
                                selectedSkill?.name === skill.name
                                  ? "bg-primary"
                                  : "bg-muted-foreground/40",
                              )}
                            />
                            <span className="text-sm font-medium">
                              {skill.name}
                            </span>
                          </div>
                          <span className="ml-3.5 text-xs text-muted-foreground line-clamp-2">
                            {skill.description}
                          </span>
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              </>
            )}
          </div>
        </>
      )}
    </div>
  );
}

// Skill chip component for displaying selected skill
export interface SkillChipProps {
  skill: Skill;
  onRemove: () => void;
  disabled?: boolean;
}

export function SkillChip({ skill, onRemove, disabled = false }: SkillChipProps) {
  return (
    <div className="inline-flex items-center gap-1 rounded-full bg-primary/10 px-3 py-1 text-sm text-primary">
      <Sparkles className="h-3 w-3" />
      <span className="font-medium">{skill.name}</span>
      <button
        onClick={onRemove}
        disabled={disabled}
        className="ml-1 rounded-full p-0.5 hover:bg-primary/20 disabled:opacity-50"
        title="Remove skill"
      >
        <X className="h-3 w-3" />
      </button>
    </div>
  );
}
