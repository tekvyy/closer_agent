import { Button } from "@/components/ui/button";
import { IconLoader2 } from "@tabler/icons-react";

interface EmptyStateProps {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  description: string;
  action?: {
    label: string;
    loadingLabel?: string;
    onClick: () => void;
    isLoading?: boolean;
    icon?: React.ComponentType<{ className?: string }>;
  };
}

export function EmptyState({ icon: Icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center">
      <div className="size-12 rounded-full bg-white/5 flex items-center justify-center mb-4">
        <Icon className="size-6 text-muted-foreground" />
      </div>
      <h3 className="text-sm font-medium mb-1">{title}</h3>
      <p className="text-sm text-muted-foreground max-w-sm mb-4">{description}</p>
      {action && (
        <Button onClick={action.onClick} disabled={action.isLoading}>
          {action.isLoading ? (
            <IconLoader2 className="size-4 animate-spin" />
          ) : action.icon ? (
            <action.icon className="size-4" />
          ) : null}
          {action.isLoading ? action.loadingLabel || "Starting..." : action.label}
        </Button>
      )}
    </div>
  );
}

interface SmallEmptyStateProps {
  icon: React.ComponentType<{ className?: string }>;
  message: string;
}

export function SmallEmptyState({ icon: Icon, message }: SmallEmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-12 text-center">
      <div className="size-10 rounded-full bg-white/5 flex items-center justify-center mb-3">
        <Icon className="size-5 text-muted-foreground" />
      </div>
      <p className="text-sm text-muted-foreground">{message}</p>
    </div>
  );
}
