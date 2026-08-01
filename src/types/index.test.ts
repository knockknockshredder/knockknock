import { describe, expect, it } from "vitest";
import type {
  BatchRootResult,
  ChildErrorDto,
  ExecuteRootRequest,
  ExecuteRootsRequest,
  ExecutionStage,
  RootResultDto,
  RootStatus,
  TargetAvailability,
  TargetKind,
  TargetMetadataDto,
  VaultSchemaSource,
  VaultTarget,
} from "./index";

describe("root execution contract", () => {
  it("exposes every snake-case enum value", () => {
    const targetKinds: TargetKind[] = ["file", "directory", "link", "unknown_legacy"];
    const availability: TargetAvailability[] = ["ready", "missing", "blocked"];
    const statuses: RootStatus[] = ["destroyed", "failed", "cancelled", "skipped"];
    const stages: ExecutionStage[] = [
      "preflight",
      "overwrite",
      "verify",
      "rename",
      "truncate",
      "delete",
      "directory_remove",
      "journal",
      "sync",
    ];
    const schemaSources: VaultSchemaSource[] = ["v1", "v2"];

    expect(targetKinds).toEqual(["file", "directory", "link", "unknown_legacy"]);
    expect(availability).toEqual(["ready", "missing", "blocked"]);
    expect(statuses).toEqual(["destroyed", "failed", "cancelled", "skipped"]);
    expect(stages).toEqual([
      "preflight",
      "overwrite",
      "verify",
      "rename",
      "truncate",
      "delete",
      "directory_remove",
      "journal",
      "sync",
    ]);
    expect(schemaSources).toEqual(["v1", "v2"]);
  });

  it("round-trips every DTO fixture without changing its shape", () => {
    const target: VaultTarget = { path: "C:\\selected\\root", kind: "directory" };
    const metadata: TargetMetadataDto = {
      path: target.path,
      kind: target.kind,
      availability: "ready",
      reason: null,
      name: "root",
      size: 42,
    };
    const request: ExecuteRootRequest = {
      target_id: "target-1",
      path: target.path,
      kind: target.kind,
    };
    const requests: ExecuteRootsRequest = { roots: [request] };
    const error: ChildErrorDto = {
      path: "C:\\selected\\root\\child",
      stage: "verify",
      error_type: "verification_failed",
      message: "verification failed",
      actionable: "Retry the operation",
    };
    const result: RootResultDto = {
      target_id: "target-1",
      requested_path: target.path,
      kind: target.kind,
      status: "failed",
      root_removed: false,
      files_destroyed: 1,
      directories_removed: 0,
      bytes_shredded: 42,
      errors: [error],
    };
    const batch: BatchRootResult = { roots: [result] };

    for (const fixture of [target, metadata, request, requests, error, result, batch]) {
      expect(JSON.parse(JSON.stringify(fixture))).toEqual(fixture);
    }
  });

  it("keeps unknown enum values outside the typed fixtures", () => {
    const unknownValues = ["unknown", "v3", "not_a_stage"];
    const knownValues = new Set([
      "file",
      "directory",
      "link",
      "unknown_legacy",
      "ready",
      "missing",
      "blocked",
      "destroyed",
      "failed",
      "cancelled",
      "skipped",
      "preflight",
      "overwrite",
      "verify",
      "rename",
      "truncate",
      "delete",
      "directory_remove",
      "journal",
      "sync",
      "v1",
      "v2",
    ]);

    expect(unknownValues.every((value) => !knownValues.has(value))).toBe(true);
  });
});
