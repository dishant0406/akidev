// The single REST client used by the whole app.
//
// The API is JSON over HTTP (apps/server/API.md), so this is a small fetch wrapper: it
// also keeps the request count and the bundle honest.

export const API_BASE = (process.env.NEXT_PUBLIC_API_URL || "http://localhost:9000").replace(/\/+$/, "");

// Finding a source searches torrent indexers and can legitimately take a while, but it must
// never hang. The server bounds its own work to about a minute, so this sits above that and
// turns a stuck request into a visible error instead of a permanent spinner.
const TIMEOUT_MS = Number(process.env.NEXT_PUBLIC_API_TIMEOUT_MS) || 90000;

/** A failed response: the server's `{ error }` message and the HTTP status. */
export class ApiError extends Error {
  readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

export type Params = Record<string, string | number | boolean | null | undefined>;

export async function request<T>(
  path: string,
  options: { method?: "GET" | "POST"; params?: Params; body?: unknown; signal?: AbortSignal } = {},
): Promise<T> {
  const query = new URLSearchParams();
  for (const [name, value] of Object.entries(options.params ?? {})) {
    if (value !== undefined && value !== null && value !== "") query.set(name, String(value));
  }
  const url = `${API_BASE}${path}${query.size > 0 ? `?${query}` : ""}`;

  const timeout = AbortSignal.timeout(TIMEOUT_MS);
  const controller = new AbortController();
  // First abort wins; caller cancellation is distinct from a request timeout.
  const cancel = () => controller.abort(new DOMException("Request cancelled", "AbortError"));
  const timedOut = () => controller.abort(timeout.reason);
  if (options.signal?.aborted) cancel();
  else options.signal?.addEventListener("abort", cancel, { once: true });
  if (timeout.aborted) timedOut();
  else timeout.addEventListener("abort", timedOut, { once: true });

  try {
    controller.signal.throwIfAborted();
    const response = await fetch(url, {
      method: options.method ?? "GET",
      headers: options.body === undefined ? undefined : { "Content-Type": "application/json" },
      body: options.body === undefined ? undefined : JSON.stringify(options.body),
      signal: controller.signal,
    });

    if (!response.ok) {
      const body: unknown = await response.json().catch(() => null);
      controller.signal.throwIfAborted();
      const message = body && typeof body === "object" && "error" in body ? (body as { error: unknown }).error : null;
      throw new ApiError(
        typeof message === "string" && message ? message : `The Akidev API returned ${response.status}.`,
        response.status,
      );
    }

    const body = (await response.json()) as T;
    controller.signal.throwIfAborted();
    return body;
  } catch (error) {
    if (controller.signal.aborted && controller.signal.reason?.name === "AbortError") {
      throw controller.signal.reason;
    }
    if (timeout.aborted || (error instanceof Error && (error.name === "TimeoutError" || error.name === "AbortError"))) {
      throw new Error("The Akidev API took too long to respond.");
    }
    if (error instanceof ApiError) throw error;
    throw new Error("Could not reach the Akidev API.");
  } finally {
    options.signal?.removeEventListener("abort", cancel);
    timeout.removeEventListener("abort", timedOut);
  }
}

export function get<T>(path: string, params?: Params, signal?: AbortSignal): Promise<T> {
  return request<T>(path, { params, signal });
}

export function post<T>(path: string, body: unknown, signal?: AbortSignal): Promise<T> {
  return request<T>(path, { method: "POST", body, signal });
}
