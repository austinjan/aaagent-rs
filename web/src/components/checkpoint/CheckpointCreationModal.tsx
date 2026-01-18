import { useState, useCallback } from "react";
import { Bookmark, Loader2, AlertCircle, CheckCircle2 } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import type { CompressionStrategy, PreviewCheckpointResponse } from "@/types/backend";
import { createCheckpoint, previewCheckpoint } from "@/services/api";

type ModalStep = "configure" | "preview" | "creating" | "success" | "error";

interface CheckpointCreationModalProps {
  isOpen: boolean;
  sessionId: string;
  targetNodeId: string;
  nodeCount: number;
  estimatedTokens: number;
  hasPreviousCheckpoint: boolean;
  onClose: () => void;
  onCheckpointCreated: (checkpointId: string) => void;
}

export function CheckpointCreationModal({
  isOpen,
  sessionId,
  targetNodeId,
  nodeCount,
  estimatedTokens,
  hasPreviousCheckpoint,
  onClose,
  onCheckpointCreated,
}: CheckpointCreationModalProps) {
  const [strategy, setStrategy] = useState<CompressionStrategy>("balanced");
  const [customPrompt, setCustomPrompt] = useState("");
  const [useMainProvider, setUseMainProvider] = useState(false);
  const [step, setStep] = useState<ModalStep>("configure");
  const [summaryPreview, setSummaryPreview] = useState<string | null>(null);
  const [previewStats, setPreviewStats] = useState<PreviewCheckpointResponse["stats"] | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isLoadingPreview, setIsLoadingPreview] = useState(false);

  const resetState = useCallback(() => {
    setStrategy("balanced");
    setCustomPrompt("");
    setUseMainProvider(false);
    setStep("configure");
    setSummaryPreview(null);
    setPreviewStats(null);
    setErrorMessage(null);
    setIsLoadingPreview(false);
  }, []);

  const handleClose = useCallback(() => {
    if (step === "creating") {
      // Could show confirmation dialog here
      return;
    }
    resetState();
    onClose();
  }, [step, resetState, onClose]);

  const handlePreview = useCallback(async () => {
    setIsLoadingPreview(true);
    setErrorMessage(null);

    try {
      const response = await previewCheckpoint(sessionId, {
        target_node_id: targetNodeId,
        strategy,
        custom_prompt: strategy === "custom" ? customPrompt : undefined,
        use_main_provider: useMainProvider,
      });
      setSummaryPreview(response.summary);
      setPreviewStats(response.stats);
      setStep("preview");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to generate preview";
      setErrorMessage(message);
    } finally {
      setIsLoadingPreview(false);
    }
  }, [sessionId, targetNodeId, strategy, customPrompt, useMainProvider]);

  const handleConfirm = useCallback(async () => {
    setStep("creating");
    setErrorMessage(null);

    try {
      const response = await createCheckpoint(sessionId, {
        target_node_id: targetNodeId,
        strategy,
        custom_prompt: strategy === "custom" ? customPrompt : undefined,
        use_main_provider: useMainProvider,
      });
      setStep("success");
      setTimeout(() => {
        onCheckpointCreated(response.checkpoint_id);
        handleClose();
      }, 1500);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to create checkpoint";
      setErrorMessage(message);
      setStep("error");
    }
  }, [sessionId, targetNodeId, strategy, customPrompt, useMainProvider, onCheckpointCreated, handleClose]);

  const canConfirm = strategy !== "custom" || customPrompt.trim().length > 0;

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && handleClose()}>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Bookmark className="h-5 w-5 text-[hsl(var(--role-checkpoint))]" />
            Create Checkpoint
          </DialogTitle>
          <DialogDescription>
            Compress {nodeCount ?? 0} messages (~{(estimatedTokens ?? 0).toLocaleString()} tokens) into a summary.
            {hasPreviousCheckpoint && (
              <span className="block mt-1 text-xs text-muted-foreground">
                Note: This will compress from the previous checkpoint.
              </span>
            )}
          </DialogDescription>
        </DialogHeader>

        {/* Configure Step */}
        {(step === "configure" || step === "preview") && (
          <div className="space-y-4 py-2">
            {/* Strategy Selection */}
            <div className="space-y-2">
              <Label className="text-sm font-medium">Compression Strategy</Label>
              <div className="grid grid-cols-3 gap-2">
                <StrategyButton
                  label="Balanced"
                  description="Keep key facts and decisions"
                  isSelected={strategy === "balanced"}
                  onClick={() => setStrategy("balanced")}
                />
                <StrategyButton
                  label="Aggressive"
                  description="Minimal, outcomes only"
                  isSelected={strategy === "aggressive"}
                  onClick={() => setStrategy("aggressive")}
                />
                <StrategyButton
                  label="Custom"
                  description="Your own prompt"
                  isSelected={strategy === "custom"}
                  onClick={() => setStrategy("custom")}
                />
              </div>
            </div>

            {/* Custom Prompt */}
            {strategy === "custom" && (
              <div className="space-y-2">
                <Label htmlFor="custom-prompt" className="text-sm font-medium">
                  Custom Compression Prompt
                </Label>
                <Textarea
                  id="custom-prompt"
                  placeholder="Describe how you want the conversation summarized..."
                  value={customPrompt}
                  onChange={(e) => setCustomPrompt(e.target.value)}
                  className="min-h-[80px] resize-none"
                />
              </div>
            )}

            {/* Provider Selection */}
            <div className="flex items-center justify-between py-2">
              <div className="space-y-0.5">
                <Label htmlFor="use-main-provider" className="text-sm font-medium">
                  Use Main Model
                </Label>
                <p className="text-xs text-muted-foreground">
                  Use the session's main model instead of the quick provider
                </p>
              </div>
              <Switch
                id="use-main-provider"
                checked={useMainProvider}
                onCheckedChange={setUseMainProvider}
              />
            </div>

            {/* Preview Section */}
            {step === "preview" && summaryPreview && (
              <div className="space-y-2">
                <Label className="text-sm font-medium">Preview</Label>
                <div className={cn(
                  "p-3 rounded-md text-sm",
                  "bg-[hsl(var(--role-checkpoint)/0.08)]",
                  "border border-[hsl(var(--role-checkpoint)/0.25)]",
                  "max-h-[200px] overflow-y-auto"
                )}>
                  {summaryPreview}
                </div>
                {previewStats && (
                  <div className="flex gap-4 text-xs text-muted-foreground">
                    <span>Original: {previewStats.original_tokens?.toLocaleString() ?? 0} tokens</span>
                    <span>Summary: {previewStats.estimated_summary_tokens?.toLocaleString() ?? 0} tokens</span>
                    <span className="text-green-600">
                      {Math.round((1 - (previewStats.estimated_compression_ratio ?? 0)) * 100)}% reduced
                    </span>
                  </div>
                )}
              </div>
            )}

            {/* Error Message */}
            {errorMessage && (
              <div className="flex items-center gap-2 p-2 rounded-md bg-red-50 text-red-600 text-sm">
                <AlertCircle className="h-4 w-4 flex-shrink-0" />
                <span>{errorMessage}</span>
              </div>
            )}
          </div>
        )}

        {/* Creating Step */}
        {step === "creating" && (
          <div className="flex flex-col items-center justify-center py-8 gap-4">
            <Loader2 className="h-8 w-8 animate-spin text-[hsl(var(--role-checkpoint))]" />
            <p className="text-sm text-muted-foreground">Creating checkpoint...</p>
          </div>
        )}

        {/* Success Step */}
        {step === "success" && (
          <div className="flex flex-col items-center justify-center py-8 gap-4">
            <CheckCircle2 className="h-8 w-8 text-green-600" />
            <p className="text-sm text-green-600 font-medium">Checkpoint created successfully!</p>
          </div>
        )}

        {/* Error Step */}
        {step === "error" && (
          <div className="flex flex-col items-center justify-center py-8 gap-4">
            <AlertCircle className="h-8 w-8 text-red-600" />
            <p className="text-sm text-red-600">{errorMessage}</p>
          </div>
        )}

        {/* Footer */}
        <DialogFooter>
          {(step === "configure" || step === "preview") && (
            <>
              <Button variant="outline" onClick={handleClose}>
                Cancel
              </Button>
              {step === "configure" && (
                <Button
                  variant="outline"
                  onClick={handlePreview}
                  disabled={!canConfirm || isLoadingPreview}
                >
                  {isLoadingPreview ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin mr-2" />
                      Loading...
                    </>
                  ) : (
                    "Preview"
                  )}
                </Button>
              )}
              {step === "preview" && (
                <Button
                  variant="outline"
                  onClick={() => setStep("configure")}
                >
                  Edit
                </Button>
              )}
              <Button
                onClick={handleConfirm}
                disabled={!canConfirm}
                className="bg-[hsl(var(--role-checkpoint))] hover:bg-[hsl(var(--role-checkpoint)/0.9)] text-black"
              >
                Create Checkpoint
              </Button>
            </>
          )}
          {step === "error" && (
            <>
              <Button variant="outline" onClick={handleClose}>
                Cancel
              </Button>
              <Button onClick={() => setStep("configure")}>
                Try Again
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

interface StrategyButtonProps {
  label: string;
  description: string;
  isSelected: boolean;
  onClick: () => void;
}

function StrategyButton({ label, description, isSelected, onClick }: StrategyButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex flex-col items-start p-2 rounded-md border text-left transition-colors",
        isSelected
          ? "border-[hsl(var(--role-checkpoint))] bg-[hsl(var(--role-checkpoint)/0.1)]"
          : "border-border hover:border-[hsl(var(--role-checkpoint)/0.5)] hover:bg-muted/50"
      )}
    >
      <span className={cn(
        "text-sm font-medium",
        isSelected && "text-[hsl(var(--role-checkpoint))]"
      )}>
        {label}
      </span>
      <span className="text-xs text-muted-foreground line-clamp-1">
        {description}
      </span>
    </button>
  );
}

export default CheckpointCreationModal;
