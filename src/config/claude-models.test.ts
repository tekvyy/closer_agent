import { describe, expect, test } from "bun:test";

import modelConfig from "./claude-models.json";

const EXPECTED_SUPPORTED_MODELS = [
  "claude-fable-5-1",
  "claude-opus-5-5",
  "claude-sonnet-5-5",
  "claude-haiku-5-5",
  "gpt-6-astra",
  "gpt-6.1-sol",
  "gpt-6-luna",
];

describe("Model configuration", () => {
  test("contains every supported Claude and ChatGPT model", () => {
    expect(modelConfig.models.map((model) => model.value)).toEqual(EXPECTED_SUPPORTED_MODELS);
  });

  test("uses a configured model as the default", () => {
    expect(EXPECTED_SUPPORTED_MODELS).toContain(modelConfig.defaultModel);
  });

  test("assigns every model to a known provider", () => {
    for (const model of modelConfig.models) {
      expect(["claude", "codex"]).toContain(model.provider);
    }
  });

  test("has unique identifiers and display labels", () => {
    expect(new Set(modelConfig.models.map((model) => model.value)).size).toBe(
      modelConfig.models.length
    );
    expect(new Set(modelConfig.models.map((model) => model.label)).size).toBe(
      modelConfig.models.length
    );
  });
});
