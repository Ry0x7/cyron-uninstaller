import { describe, it, expect } from "vitest";
import {
  RISK_DESCRIPTION,
  RISK_LABEL,
  RISK_ORDER,
  RISK_SHORT,
  RISK_TONE,
  isRisk,
  normalizeRisk,
  riskRank,
} from "./risk";

describe("risk vocabulary", () => {
  it("orders risk from safest to strictest", () => {
    expect(RISK_ORDER).toEqual(["safe", "caution", "protected"]);
    expect(riskRank("safe")).toBeLessThan(riskRank("caution"));
    expect(riskRank("caution")).toBeLessThan(riskRank("protected"));
  });

  it("recognises only known levels", () => {
    expect(isRisk("safe")).toBe(true);
    expect(isRisk("caution")).toBe(true);
    expect(isRisk("protected")).toBe(true);
    expect(isRisk("unknown")).toBe(false);
    expect(isRisk("")).toBe(false);
    expect(normalizeRisk("bogus")).toBe("caution");
    expect(normalizeRisk("safe")).toBe("safe");
  });

  it("has plain language for every level", () => {
    for (const risk of RISK_ORDER) {
      expect(RISK_LABEL[risk].length).toBeGreaterThan(0);
      expect(RISK_SHORT[risk].length).toBeGreaterThan(0);
      expect(RISK_DESCRIPTION[risk].length).toBeGreaterThan(20);
    }
    expect(RISK_LABEL.protected).toBe("Protected");
  });

  it("maps levels to badge tones", () => {
    expect(RISK_TONE).toEqual({ safe: "safe", caution: "caution", protected: "neutral" });
  });
});
