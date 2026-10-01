import type { Engine } from "@/types/domain";

// Etiqueta corta del motor para chips y hechos (MZ, VX Ace…).
export function engineLabel(engine: Engine): string {
  switch (engine) {
    case "mv":
      return "MV";
    case "mz":
      return "MZ";
    case "vxAce":
      return "VX Ace";
    case "vx":
      return "VX";
    case "xp":
      return "XP";
    case "unity":
      return "Unity JSON";
  }
}

// Nombre completo del motor para el diálogo de nuevo proyecto (RPG Maker MZ…).
export function engineName(engine: Engine): string {
  return engine === "unity" ? "Unity (Loose JSON)" : `RPG Maker ${engineLabel(engine)}`;
}
