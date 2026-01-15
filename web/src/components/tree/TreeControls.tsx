// TreeControls component - Control panel for tree visualization settings

import { Eye, EyeOff, Maximize2 } from 'lucide-react';
import { Button } from '../ui/button';
import { Switch } from '../ui/switch';
import { Label } from '../ui/label';

export interface TreeControlsProps {
  showInactive: boolean;
  onToggleInactive: () => void;
  onCenterTree: () => void;
}

export function TreeControls({
  showInactive,
  onToggleInactive,
  onCenterTree,
}: TreeControlsProps) {
  return (
    <div className="tree-controls border-b border-border bg-background p-3 space-y-2">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-foreground">Tree Navigation</h3>
        <Button
          variant="ghost"
          size="sm"
          onClick={onCenterTree}
          title="Center tree view"
        >
          <Maximize2 className="h-4 w-4" />
        </Button>
      </div>

      {/* Toggle inactive branches */}
      <div className="flex items-center justify-between">
        <Label htmlFor="show-inactive" className="text-sm text-muted-foreground flex items-center gap-2">
          {showInactive ? <Eye className="h-4 w-4" /> : <EyeOff className="h-4 w-4" />}
          Show inactive branches
        </Label>
        <Switch
          id="show-inactive"
          checked={showInactive}
          onCheckedChange={onToggleInactive}
        />
      </div>
    </div>
  );
}
