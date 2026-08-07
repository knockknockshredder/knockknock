import { describe, expect, expectTypeOf, it } from "vitest";
import type {
  BatchRootResult,
  ChildErrorDto,
  DeletionMethod,
  ExecuteRootRequest,
  ExecuteRootsRequest,
  ExecutionStage,
  OverwriteStatus,
  RootResultDto,
  RootStatus,
  ShredStatus,
  TargetAvailability,
  TargetKind,
  TargetMetadataDto,
  VaultSchemaSource,
  VaultTarget,
  WriteCheck,
  WriteCheckOutcome,
} from "./index";

describe("root execution contract", () => {
  it("defines exact snake-case enum unions", () => {
    expectTypeOf<TargetKind>().toEqualTypeOf<
      "file" | "directory" | "link" | "unknown_legacy"
    >();
    expectTypeOf<TargetAvailability>().toEqualTypeOf<"ready" | "missing" | "blocked">();
    expectTypeOf<RootStatus>().toEqualTypeOf<
      "destroyed" | "failed" | "cancelled" | "skipped"
    >();
    expectTypeOf<ExecutionStage>().toEqualTypeOf<
      | "preflight"
      | "overwrite"
      | "verify"
      | "rename"
      | "truncate"
      | "delete"
      | "directory_remove"
      | "journal"
      | "sync"
    >();
    expectTypeOf<VaultSchemaSource>().toEqualTypeOf<"v1" | "v2">();
    expectTypeOf<DeletionMethod>().toEqualTypeOf<
      "automatic" | "legacy_three_pass"
    >();
    expectTypeOf<WriteCheck>().toEqualTypeOf<"off" | "spot" | "full">();
    expectTypeOf<WriteCheckOutcome>().toEqualTypeOf<
      "not_run" | "passed" | "failed"
    >();
    expectTypeOf<OverwriteStatus>().toEqualTypeOf<
      "not_started" | "partial" | "completed"
    >();
    expectTypeOf<ShredStatus>().toEqualTypeOf<
      | { type: "Shredding" }
      | { type: "Complete" }
      | { type: "Warning"; message: string }
      | { type: "Error"; message: string }
    >();
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
      write_check: "failed",
      errors: [error],
    };
    const batch: BatchRootResult = { roots: [result] };

    for (const fixture of [target, metadata, request, requests, error, result, batch]) {
      expect(JSON.parse(JSON.stringify(fixture))).toEqual(fixture);
    }
  });

  it("rejects invalid enum literals", () => {
    expectTypeOf<"unknown">().not.toMatchTypeOf<TargetKind>();
    expectTypeOf<"unavailable">().not.toMatchTypeOf<TargetAvailability>();
    expectTypeOf<"pending">().not.toMatchTypeOf<RootStatus>();
    expectTypeOf<"cleanup">().not.toMatchTypeOf<ExecutionStage>();
    expectTypeOf<"v3">().not.toMatchTypeOf<VaultSchemaSource>();
  });
});
