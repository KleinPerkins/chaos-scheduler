/**
 * Error raised when the Chaos Scheduler API returns a non-2xx response.
 *
 * The backend renders errors as `{ "error": "<message>" }` with a matching
 * HTTP status (`api.rs::ApiError`): 400 validation, 401 auth, 403 scope,
 * 404 not-found, 429 rate-limit, 500 internal.
 */
export class ChaosApiError extends Error {
  readonly status: number;
  readonly url: string;
  readonly method: string;
  readonly body: unknown;

  constructor(args: {
    status: number;
    url: string;
    method: string;
    message: string;
    body?: unknown;
  }) {
    super(args.message);
    this.name = "ChaosApiError";
    this.status = args.status;
    this.url = args.url;
    this.method = args.method;
    this.body = args.body;
  }

  /** True for 401/403 (missing/invalid key or insufficient scope). */
  get isAuthError(): boolean {
    return this.status === 401 || this.status === 403;
  }

  /** True for 429 (per-key rate limit exceeded). */
  get isRateLimited(): boolean {
    return this.status === 429;
  }

  /** True for 404. */
  get isNotFound(): boolean {
    return this.status === 404;
  }
}
