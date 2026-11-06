import type { Risk } from "./types";

export const RISK_ORDER: Risk[] = ["safe", "caution", "protected"];

export const RISK_LABEL: Record<Risk, string> = {
  safe: "Safe",
  caution: "Review",
  protected: "Protected",
};

export const RISK_SHORT: Record<Risk, string> = {
  safe: "Regenerates",
  caution: "Re-download",
  protected: "Keep",
};

export const RISK_DESCRIPTION: Record<Risk, string> = {
  safe: "Disposable cache. The tool recreates it on next use.",
  caution: "Removing it costs time or a re-download, but no personal data.",
  protected: "Conversation history, models, settings or volumes. Cyron never offers this for removal.",
};

export const RISK_TONE: Record<Risk, "safe" | "caution" | "neutral"> = {
  safe: "safe",
  caution: "caution",
  protected: "neutral",
};

export function riskRank(risk: Risk): number {
  return RISK_ORDER.indexOf(risk);
}

export function isRisk(value: string): value is Risk {
  return (RISK_ORDER as string[]).includes(value);
}

export function normalizeRisk(value: string): Risk {
  return isRisk(value) ? value : "caution";
}
