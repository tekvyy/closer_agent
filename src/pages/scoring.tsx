import { useQuery } from "@tanstack/react-query";
import { ScoringConfigEditor } from "@/components/scoring/config-editor";
import { IconTargetArrow, IconLoader2 } from "@tabler/icons-react";
import { getActiveScoringConfig } from "@/lib/tauri/commands";
import { defaultScoringConfig } from "@/lib/types/scoring";
import type { RequiredCharacteristic, DemandSignifier } from "@/lib/types/scoring";

type ScoringConfigSeed = {
  id: number | null;
  name: string;
  isActive: boolean;
  requiredCharacteristics: RequiredCharacteristic[];
  demandSignifiers: DemandSignifier[];
  tierHotMin: number;
  tierWarmMin: number;
  tierNurtureMin: number;
  createdAt: string | null;
  updatedAt: string | null;
};

async function fetchScoringConfig(): Promise<ScoringConfigSeed> {
  const config = await getActiveScoringConfig();
  if (!config) {
    return { ...defaultScoringConfig, id: null, createdAt: null, updatedAt: null };
  }
  const requiredCharacteristics =
    typeof config.requiredCharacteristics === "string"
      ? JSON.parse(config.requiredCharacteristics)
      : config.requiredCharacteristics;
  const demandSignifiers =
    typeof config.demandSignifiers === "string"
      ? JSON.parse(config.demandSignifiers)
      : config.demandSignifiers;

  return {
    id: config.id,
    name: config.name,
    isActive: config.isActive,
    requiredCharacteristics,
    demandSignifiers,
    tierHotMin: config.tierHotMin,
    tierWarmMin: config.tierWarmMin,
    tierNurtureMin: config.tierNurtureMin,
    createdAt: config.createdAt ? new Date(config.createdAt).toISOString() : null,
    updatedAt: config.updatedAt ? new Date(config.updatedAt).toISOString() : null,
  };
}

export default function ScoringPage() {
  const { data: seed, isLoading } = useQuery({
    queryKey: ["scoring-config"],
    queryFn: fetchScoringConfig,
  });

  return (
    <>
      <header className="h-10 border-b border-white/5 flex items-center px-4 gap-2">
        <IconTargetArrow className="size-4" />
        <h1 className="text-sm font-medium">Scoring Configuration</h1>
      </header>
      {isLoading || !seed ? (
        <div className="flex items-center justify-center h-64">
          <IconLoader2 className="size-6 animate-spin text-muted-foreground" />
        </div>
      ) : (
        <ScoringConfigEditor seed={seed} />
      )}
    </>
  );
}
