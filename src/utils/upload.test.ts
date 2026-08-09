import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { UploadError, uploadLogs } from "./upload";

const ok = (stored: number, duplicates = 0, rejected = 0, url: string | null = "/encounter/1") => ({
  ok: true,
  status: 200,
  json: async () => ({ stored, duplicates, rejected, url }),
});

describe("uploadLogs", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((_cmd: string, args: { id: number }) => Promise.resolve({ id: args.id }));
  });

  it("does nothing at all for an empty selection", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    expect(await uploadLogs([], "2.0.3")).toEqual({ stored: 0, duplicates: 0, rejected: 0, url: null });
    expect(fetchMock).not.toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("identifies itself as the app, which the endpoint requires", async () => {
    const fetchMock = vi.fn(async () => ok(1));
    vi.stubGlobal("fetch", fetchMock);

    await uploadLogs([7], "2.0.3-1");

    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("https://relink.cleista.cc/api/upload");
    expect((init.headers as Record<string, string>)["X-GBFR-Logs"]).toBe("2.0.3-1");
    expect(JSON.parse(init.body as string)).toEqual({ logs: [{ id: 7 }] });
  });

  it("splits past the server's batch cap rather than being refused wholesale", async () => {
    const fetchMock = vi.fn(async () => ok(25));
    vi.stubGlobal("fetch", fetchMock);

    const ids = Array.from({ length: 60 }, (_, i) => i + 1);
    const outcome = await uploadLogs(ids, "2.0.3");

    expect(fetchMock).toHaveBeenCalledTimes(3);
    const sizes = (fetchMock.mock.calls as unknown as Array<[string, RequestInit]>).map(
      ([, init]) => JSON.parse(init.body as string).logs.length
    );
    expect(sizes).toEqual([25, 25, 10]);
    expect(invoke).toHaveBeenCalledTimes(60);
    expect(outcome.stored).toBe(75); // the stubbed per-batch count, summed
  });

  it("keeps the FIRST accepted url across batches, and totals the counts", async () => {
    const responses = [ok(2, 1, 0, "/encounter/11"), ok(0, 3, 1, "/encounter/99")];
    const fetchMock = vi.fn(async () => responses.shift()!);
    vi.stubGlobal("fetch", fetchMock);

    const outcome = await uploadLogs(
      Array.from({ length: 30 }, (_, i) => i + 1),
      "2.0.3"
    );
    expect(outcome).toEqual({
      stored: 2,
      duplicates: 4,
      rejected: 1,
      url: "https://relink.cleista.cc/encounter/11",
    });
  });

  it("reports a url of null when nothing was accepted", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ok(0, 0, 1, null))
    );
    expect((await uploadLogs([1], "2.0.3")).url).toBeNull();
  });

  it("carries the status through, so a rate limit reads differently from a failure", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({ ok: false, status: 429, json: async () => ({}) }))
    );
    await expect(uploadLogs([1], "2.0.3")).rejects.toMatchObject({ status: 429 });
  });

  it("marks a request that never got a response as status 0", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new TypeError("Failed to fetch");
      })
    );
    const error = await uploadLogs([1], "2.0.3").catch((e) => e);
    expect(error).toBeInstanceOf(UploadError);
    expect(error.status).toBe(0);
  });
});
