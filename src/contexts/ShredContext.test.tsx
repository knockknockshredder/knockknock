import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ShredProvider, useShred } from "./ShredContext";
import type {
  TargetAvailability,
  TargetKind,
  TargetMetadataDto,
  VaultTarget,
} from "@/types";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

interface VaultLoadDto {
  source_schema: "v1" | "v2";
  migration_required: boolean;
  targets: VaultTarget[];
}

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason?: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });
  return { promise, resolve, reject };
}

function target(path: string, kind: TargetKind = "file"): VaultTarget {
  return { path, kind };
}

function metadata(
  path: string,
  kind: TargetKind = "file",
  availability: TargetAvailability = "ready"
): TargetMetadataDto {
  return {
    path,
    kind,
    availability,
    reason: availability === "ready" ? null : `${availability} target`,
    name: path.split("\\")[path.split("\\").length - 1] ?? path,
    size: 1,
  };
}

function saveCalls(): Array<{
  targets: VaultTarget[];
  pin: string;
}> {
  return invokeMock.mock.calls
    .filter(([command]) => command === "save_vault")
    .map(([, args]) => args);
}

let latest: ReturnType<typeof useShred>;

function Probe() {
  latest = useShred();
  return <output data-testid="vault-state">{latest.vaultState}</output>;
}

function renderContext() {
  return render(
    <ShredProvider>
      <Probe />
    </ShredProvider>
  );
}

function readyFile(path: string) {
  return {
    path,
    name: path.split("\\")[path.split("\\").length - 1] ?? path,
    size: 1,
    is_shortcut: false,
    shortcut_target: null,
  };
}

function configureLoadedVault(
  sourceSchema: "v1" | "v2",
  targets: VaultTarget[],
  validationTargets: TargetMetadataDto[] = targets.map(({ path, kind }) =>
    metadata(path, kind)
  )
) {
  invokeMock.mockImplementation((command: string) => {
    if (command === "vault_exists") return Promise.resolve(true);
    if (command === "load_vault") {
      return Promise.resolve<VaultLoadDto>({
        source_schema: sourceSchema,
        migration_required: sourceSchema === "v1",
        targets,
      });
    }
    if (command === "validate_targets") {
      return Promise.resolve(validationTargets);
    }
    return Promise.resolve(undefined);
  });
}

describe("authoritative vault writer", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(false);
      return Promise.resolve(undefined);
    });
  });

  it("saves the first file when no vault exists", async () => {
    renderContext();

    await act(async () => {
      await latest.loadVault("pin");
    });
    await act(async () => {
      latest.addFiles([readyFile("C:\\first.txt")]);
    });

    await waitFor(() => expect(saveCalls()).toHaveLength(1));
    expect(saveCalls()[0]).toEqual({
      targets: [target("C:\\first.txt")],
      pin: "pin",
    });
  });

  it("replaces loaded files instead of merging them", async () => {
    renderContext();
    configureLoadedVault("v2", [target("C:\\old.txt")]);

    await act(async () => {
      await latest.loadVault("pin");
    });
    expect(latest.files.map((file) => file.path)).toEqual(["C:\\old.txt"]);

    configureLoadedVault("v2", [target("C:\\new.txt")]);
    await act(async () => {
      await latest.loadVault("pin");
    });

    expect(latest.files.map((file) => file.path)).toEqual(["C:\\new.txt"]);
  });

  it("coalesces revisions and keeps at most one save in flight", async () => {
    renderContext();
    await act(async () => {
      await latest.loadVault("pin");
    });
    const firstSave = deferred<void>();
    invokeMock.mockImplementation((command: string) =>
      command === "save_vault" ? firstSave.promise : Promise.resolve(undefined)
    );

    await act(async () => {
      latest.addFiles([readyFile("C:\\first.txt")]);
    });
    await waitFor(() => expect(saveCalls()).toHaveLength(1));
    await act(async () => {
      latest.addFiles([readyFile("C:\\second.txt")]);
    });

    expect(saveCalls()).toHaveLength(1);
    await act(async () => {
      firstSave.resolve();
    });
    await waitFor(() => expect(saveCalls()).toHaveLength(2));
    expect(saveCalls()[1].targets.map(({ path }) => path)).toEqual([
      "C:\\first.txt",
      "C:\\second.txt",
    ]);
  });

  it("flushes the revision observed at call time", async () => {
    renderContext();
    await act(async () => {
      await latest.loadVault("pin");
    });
    const save = deferred<void>();
    invokeMock.mockImplementation((command: string) =>
      command === "save_vault" ? save.promise : Promise.resolve(undefined)
    );

    await act(async () => {
      latest.addFiles([readyFile("C:\\flush.txt")]);
    });
    await waitFor(() => expect(saveCalls()).toHaveLength(1));
    let flushed = false;
    const flushPromise = latest.flushVault().then(() => {
      flushed = true;
    });
    await Promise.resolve();
    expect(flushed).toBe(false);

    await act(async () => {
      save.resolve();
      await flushPromise;
    });
    expect(flushed).toBe(true);
  });

  it("rejects a flush when the PIN epoch changes in flight", async () => {
    renderContext();
    await act(async () => {
      await latest.loadVault("pin");
    });
    const save = deferred<void>();
    invokeMock.mockImplementation((command: string) =>
      command === "save_vault" ? save.promise : Promise.resolve(undefined)
    );

    await act(async () => {
      latest.addFiles([readyFile("C:\\epoch.txt")]);
    });
    await waitFor(() => expect(saveCalls()).toHaveLength(1));

    let flushPromise!: Promise<void>;
    await act(async () => {
      flushPromise = latest.flushVault();
      await Promise.resolve();
    });
    await act(async () => {
      latest.setVaultPin(null);
      save.resolve();
      await expect(flushPromise).rejects.toThrow(/epoch/i);
    });
  });

  it("flushes before PIN rekey and blocks stale old-PIN writes", async () => {
    renderContext();
    await act(async () => {
      await latest.loadVault("old-pin");
    });
    const firstOldSave = deferred<void>();
    const newestOldSave = deferred<void>();
    const rekey = deferred<{ durability_warning: string | null }>();
    let saveAttempt = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "save_vault") {
        saveAttempt += 1;
        if (saveAttempt === 1) return firstOldSave.promise;
        if (saveAttempt === 2) return newestOldSave.promise;
        return Promise.resolve(undefined);
      }
      if (command === "change_pin") return rekey.promise;
      return Promise.resolve(undefined);
    });

    await act(async () => {
      latest.addFiles([readyFile("C:\\before-rekey.txt")]);
    });
    await waitFor(() => expect(saveCalls()).toHaveLength(1));

    let changePromise!: ReturnType<typeof latest.changeVaultPin>;
    await act(async () => {
      changePromise = latest.changeVaultPin("old-pin", "new-pin");
      await Promise.resolve();
    });
    firstOldSave.resolve();
    await waitFor(() => expect(saveCalls()).toHaveLength(2));
    expect(saveCalls()[1].pin).toBe("old-pin");
    newestOldSave.resolve();
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("change_pin", {
        oldPin: "old-pin",
        newPin: "new-pin",
      })
    );
    expect(saveCalls()).toHaveLength(2);
    await act(async () => {
      latest.addFiles([readyFile("C:\\during-rekey.txt")]);
    });
    expect(saveCalls()).toHaveLength(2);

    rekey.resolve({ durability_warning: "Vault committed; parent durability sync failed" });
    let changeOutcome!: { durability_warning: string | null };
    await act(async () => {
      changeOutcome = await changePromise;
    });
    expect(changeOutcome.durability_warning).toBe(
      "Vault committed; parent durability sync failed"
    );
    expect(saveCalls().length).toBeGreaterThanOrEqual(3);
    expect(saveCalls()[0].pin).toBe("old-pin");
    expect(saveCalls()[1].pin).toBe("old-pin");
    expect(saveCalls()[2].pin).toBe("new-pin");
    expect(saveCalls()[2].targets.map(({ path }) => path)).toEqual([
      "C:\\before-rekey.txt",
      "C:\\during-rekey.txt",
    ]);
  });

  it("restores ready, missing, and blocked targets without filtering", async () => {
    renderContext();
    const targets = [
      target("C:\\ready.txt"),
      target("C:\\missing.txt"),
      target("C:\\blocked.txt"),
    ];
    configureLoadedVault("v2", targets, [
      metadata("C:\\ready.txt", "file", "ready"),
      metadata("C:\\missing.txt", "file", "missing"),
      metadata("C:\\blocked.txt", "file", "blocked"),
    ]);

    await act(async () => {
      await latest.loadVault("pin");
    });

    expect(
      latest.files.map(({ path, status, error }) => ({ path, status, error }))
    ).toEqual([
      { path: "C:\\ready.txt", status: "pending", error: undefined },
      { path: "C:\\missing.txt", status: "error", error: "missing target" },
      { path: "C:\\blocked.txt", status: "error", error: "blocked target" },
    ]);
  });

  it("retains a failed snapshot and retries it", async () => {
    renderContext();
    await act(async () => {
      await latest.loadVault("pin");
    });
    const failedSave = deferred<void>();
    const retrySave = deferred<void>();
    let saveAttempt = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command !== "save_vault") return Promise.resolve(undefined);
      saveAttempt += 1;
      return saveAttempt === 1 ? failedSave.promise : retrySave.promise;
    });

    await act(async () => {
      latest.addFiles([readyFile("C:\\retry.txt")]);
    });
    await waitFor(() => expect(saveCalls()).toHaveLength(1));
    await act(async () => {
      failedSave.reject(new Error("write failed"));
    });
    await waitFor(() => expect(latest.vaultState).toBe("error"));

    let flushPromise!: Promise<void>;
    await act(async () => {
      flushPromise = latest.flushVault();
      await Promise.resolve();
    });
    await waitFor(() => expect(saveCalls()).toHaveLength(2));
    expect(saveCalls()[1].targets).toEqual([target("C:\\retry.txt")]);
    await act(async () => {
      retrySave.resolve();
      await flushPromise;
    });
    await waitFor(() => expect(latest.vaultState).toBe("clean"));
  });

  it("restarts the current-epoch queue when a stale save rejects", async () => {
    renderContext();
    await act(async () => {
      await latest.loadVault("old-pin");
    });
    const oldSave = deferred<void>();
    const newSave = deferred<void>();
    let saveAttempt = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command !== "save_vault") return Promise.resolve(undefined);
      saveAttempt += 1;
      return saveAttempt === 1 ? oldSave.promise : newSave.promise;
    });

    await act(async () => {
      latest.addFiles([readyFile("C:\\old.txt")]);
    });
    await waitFor(() => expect(saveCalls()).toHaveLength(1));
    await act(async () => {
      latest.addFiles([readyFile("C:\\queued-old.txt")]);
      latest.setVaultPin("new-pin");
    });
    await act(async () => {
      oldSave.reject(new Error("old PIN write failed"));
    });
    await waitFor(() => expect(saveCalls()).toHaveLength(2));
    expect(saveCalls()[1].pin).toBe("new-pin");
    expect(saveCalls()[1].targets.map(({ path }) => path)).toEqual([
      "C:\\old.txt",
      "C:\\queued-old.txt",
    ]);
    await act(async () => {
      newSave.resolve();
    });
  });

  it("waits for complete V1 validation before migrating to V2", async () => {
    renderContext();
    const targets = [target("C:\\legacy.txt")];
    const validation = deferred<ReturnType<typeof metadata>[]>();
    const migrationSave = deferred<void>();
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve<VaultLoadDto>({
          source_schema: "v1",
          migration_required: true,
          targets,
        });
      }
      if (command === "validate_targets") return validation.promise;
      if (command === "save_vault") return migrationSave.promise;
      return Promise.resolve(undefined);
    });

    let loaded = false;
    let loadPromise!: Promise<void>;
    await act(async () => {
      loadPromise = latest.loadVault("pin").then(() => {
        loaded = true;
      });
      await Promise.resolve();
    });
    await Promise.resolve();
    expect(saveCalls()).toHaveLength(0);
    expect(loaded).toBe(false);

    await act(async () => {
      validation.resolve([metadata("C:\\legacy.txt")]);
    });
    await waitFor(() => expect(saveCalls()).toHaveLength(1));
    expect(loaded).toBe(false);
    expect(saveCalls()[0]).toEqual({ targets, pin: "pin" });

    await act(async () => {
      migrationSave.resolve();
      await loadPromise;
    });
    expect(loaded).toBe(true);
    expect(latest.vaultState).toBe("clean");
  });

  it("keeps the writer locked after a failed load", async () => {
    renderContext();
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") return Promise.reject(new Error("bad PIN"));
      return Promise.resolve(undefined);
    });

    await act(async () => {
      const loadPromise = latest.loadVault("bad-pin");
      await expect(loadPromise).rejects.toThrow("bad PIN");
    });
    expect(latest.vaultState).toBe("error");
    await act(async () => {
      latest.addFiles([readyFile("C:\\must-not-save.txt")]);
    });
    expect(saveCalls()).toHaveLength(0);
  });
});
