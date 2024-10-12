/* eslint-disable */
/* tslint:disable */
/*
 * ---------------------------------------------------------------
 * ## THIS FILE WAS GENERATED VIA SWAGGER-TYPESCRIPT-API        ##
 * ##                                                           ##
 * ## AUTHOR: acacode                                           ##
 * ## SOURCE: https://github.com/acacode/swagger-typescript-api ##
 * ---------------------------------------------------------------
 */

export interface Backend {
  address: string;
  /** H2C,H2,H2CPK - used to signal use of prior knowledge http2 or http2 over clear text. */
  hints?: Hint[] | null;
  https?: boolean | null;
  /**
   * This can be zero in case the backend is a hosted process, in which case we will need to resolve the current active_port
   * @format int32
   * @min 0
   */
  port: number;
}

export enum BasicLogFormat {
  Standard = "Standard",
  Dotnet = "Dotnet",
}

export enum BasicLogLevel {
  Trace = "Trace",
  Debug = "Debug",
  Info = "Info",
  Warn = "Warn",
  Error = "Error",
}

export enum BasicProcState {
  Faulty = "Faulty",
  Stopped = "Stopped",
  Starting = "Starting",
  Stopping = "Stopping",
  Running = "Running",
  Remote = "Remote",
}

export type ConfigItem =
  | {
      RemoteSite: RemoteSiteConfig;
    }
  | {
      HostedProcess: InProcessSiteConfig;
    };

export type ConfigurationItem =
  | {
      HostedProcess: InProcessSiteConfig;
    }
  | {
      RemoteSite: RemoteSiteConfig;
    };

export interface EnvVar {
  key: string;
  value: string;
}

export interface FullyResolvedInProcessSiteConfig {
  /**
   * @format int32
   * @min 0
   */
  active_port?: number | null;
  args?: string[] | null;
  auto_start?: boolean | null;
  bin: string;
  capture_subdomains?: boolean | null;
  dir?: string | null;
  disable_tcp_tunnel_mode?: boolean | null;
  env_vars?: EnvVar[] | null;
  excluded_from_start_all: boolean;
  forward_subdomains?: boolean | null;
  hints?: Hint[] | null;
  host_name: string;
  https?: boolean | null;
  log_format?: LogFormat | null;
  /**
   * @format int32
   * @min 0
   */
  port?: number | null;
  proc_id: any;
}

export enum H2Hint {
  H2 = "H2",
  H2C = "H2C",
}

export enum Hint {
  H2 = "H2",
  H2C = "H2C",
  H2CPK = "H2CPK",
  NOH2 = "NOH2",
}

export interface InProcessSiteConfig {
  args?: string[] | null;
  /**
   * Set this to false if you do not want this site to start automatically when odd-box starts.
   * This also means that the site is excluded from the start_all command.
   */
  auto_start?: boolean | null;
  bin: string;
  /** If you wish to use wildcard routing for any subdomain under the 'host_name' */
  capture_subdomains?: boolean | null;
  dir?: string | null;
  /** This is mostly useful in case the target uses SNI sniffing/routing */
  disable_tcp_tunnel_mode?: boolean | null;
  /**
   * If you want to use lets-encrypt for generating certificates automatically for this site.
   * Defaults to false. This feature will disable tcp tunnel mode.
   */
  enable_lets_encrypt?: boolean | null;
  env_vars?: EnvVar[] | null;
  /**
   * If you wish to exclude this site from the start_all command.
   * This setting was previously called "disable" but has been renamed for clarity
   */
  exclude_from_start_all?: boolean | null;
  /**
   * If you wish to use the subdomain from the request in forwarded requests:
   * test.example.com -> internal.site
   * vs
   * test.example.com -> test.internal.site
   */
  forward_subdomains?: boolean | null;
  /** H2C or H2 - used to signal use of prior knowledge http2 or http2 over clear text. */
  hints?: Hint[] | null;
  host_name: string;
  https?: boolean | null;
  log_format?: LogFormat | null;
  /**
   * If this is set to None, the next available port will be used. Starting from the global port_range_start
   * @format int32
   * @min 0
   */
  port?: number | null;
}

export interface KvP {
  key: string;
  value: string;
}

export interface ListResponse {
  items: ConfigurationItem[];
}

export enum LogFormat {
  Standard = "standard",
  Dotnet = "dotnet",
}

export enum LogLevel {
  Trace = "Trace",
  Debug = "Debug",
  Info = "Info",
  Warn = "Warn",
  Error = "Error",
}

export interface OddBoxConfigGlobalPart {
  /**
   * @format int32
   * @min 0
   */
  admin_api_port: number;
  alpn: boolean;
  auto_start: boolean;
  default_log_format: BasicLogFormat;
  env_vars: KvP[];
  /**
   * @format int32
   * @min 0
   */
  http_port: number;
  ip: string;
  lets_encrypt_account_email: string;
  log_level: BasicLogLevel;
  path: string;
  /**
   * @format int32
   * @min 0
   */
  port_range_start: number;
  root_dir: string;
  /**
   * @format int32
   * @min 0
   */
  tls_port: number;
}

export enum OddBoxConfigVersion {
  Unmarked = "Unmarked",
  V1 = "V1",
  V2 = "V2",
}

export interface OddBoxV1Config {
  /**
   * @format int32
   * @min 0
   */
  admin_api_port?: number | null;
  /** Defaults to true. Lets you enable/disable h2/http11 tls alpn algs during initial connection phase. */
  alpn?: boolean | null;
  auto_start?: boolean | null;
  default_log_format?: LogFormat;
  env_vars: EnvVar[];
  hosted_process?: InProcessSiteConfig[] | null;
  /**
   * @format int32
   * @min 0
   */
  http_port?: number | null;
  ip: string;
  log_level?: LogLevel | null;
  path?: string | null;
  /**
   * @format int32
   * @min 0
   */
  port_range_start: number;
  remote_target?: RemoteSiteConfig[] | null;
  root_dir?: string | null;
  /**
   * @format int32
   * @min 0
   */
  tls_port?: number | null;
  version: string;
}

export interface OddBoxV2Config {
  /**
   * @format int32
   * @min 0
   */
  admin_api_port?: number | null;
  /** Defaults to true. Lets you enable/disable h2/http11 tls alpn algs during initial connection phase. */
  alpn?: boolean | null;
  auto_start?: boolean | null;
  default_log_format?: LogFormat;
  env_vars: EnvVar[];
  hosted_process?: InProcessSiteConfig[] | null;
  /**
   * @format int32
   * @min 0
   */
  http_port?: number | null;
  ip: string;
  lets_encrypt_account_email?: string | null;
  log_level?: LogLevel | null;
  path?: string | null;
  /**
   * @format int32
   * @min 0
   */
  port_range_start: number;
  remote_target?: RemoteSiteConfig[] | null;
  root_dir?: string | null;
  /**
   * @format int32
   * @min 0
   */
  tls_port?: number | null;
  version: string;
}

export enum ProcState {
  Faulty = "Faulty",
  Stopped = "Stopped",
  Starting = "Starting",
  Stopping = "Stopping",
  Running = "Running",
  Remote = "Remote",
}

export interface RemoteSiteConfig {
  backends: Backend[];
  /** If you wish to use wildcard routing for any subdomain under the 'host_name' */
  capture_subdomains?: boolean | null;
  /** This is mostly useful in case the target uses SNI sniffing/routing */
  disable_tcp_tunnel_mode?: boolean | null;
  /**
   * If you want to use lets-encrypt for generating certificates automatically for this site.
   * Defaults to false. This feature will disable tcp tunnel mode.
   */
  enable_lets_encrypt?: boolean | null;
  /**
   * If you wish to use the subdomain from the request in forwarded requests:
   * test.example.com -> internal.site
   * vs
   * test.example.com -> test.internal.site
   */
  forward_subdomains?: boolean | null;
  host_name: string;
}

export interface SaveGlobalConfig {
  /**
   * @format int32
   * @min 0
   */
  admin_api_port: number;
  alpn: boolean;
  auto_start: boolean;
  default_log_format: BasicLogFormat;
  env_vars: KvP[];
  /**
   * @format int32
   * @min 0
   */
  http_port: number;
  ip: string;
  lets_encrypt_account_email: string;
  log_level: BasicLogLevel;
  /**
   * @format int32
   * @min 0
   */
  port_range_start: number;
  root_dir: string;
  /**
   * @format int32
   * @min 0
   */
  tls_port: number;
}

export type SitesError = {
  UnknownError: string;
};

export interface StatusItem {
  hostname: string;
  state: BasicProcState;
}

export interface StatusResponse {
  items: StatusItem[];
}

export interface UpdateRequest {
  new_configuration: ConfigItem;
}

export type SettingsData = OddBoxConfigGlobalPart;

export type SaveSettingsData = any;

export type ListData = ListResponse;

/** @default null */
export type SetData = any;

export type DeleteData = any;

export type StartData = any;

export type StatusData = StatusResponse;

export type StopData = any;

export type QueryParamsType = Record<string | number, any>;
export type ResponseFormat = keyof Omit<Body, "body" | "bodyUsed">;

export interface FullRequestParams extends Omit<RequestInit, "body"> {
  /** set parameter to `true` for call `securityWorker` for this request */
  secure?: boolean;
  /** request path */
  path: string;
  /** content type of request body */
  type?: ContentType;
  /** query params */
  query?: QueryParamsType;
  /** format of response (i.e. response.json() -> format: "json") */
  format?: ResponseFormat;
  /** request body */
  body?: unknown;
  /** base url */
  baseUrl?: string;
  /** request cancellation token */
  cancelToken?: CancelToken;
}

export type RequestParams = Omit<FullRequestParams, "body" | "method" | "query" | "path">;

export interface ApiConfig<SecurityDataType = unknown> {
  baseUrl?: string;
  baseApiParams?: Omit<RequestParams, "baseUrl" | "cancelToken" | "signal">;
  securityWorker?: (securityData: SecurityDataType | null) => Promise<RequestParams | void> | RequestParams | void;
  customFetch?: typeof fetch;
}

export interface HttpResponse<D extends unknown, E extends unknown = unknown> extends Response {
  data: D;
  error: E;
}

type CancelToken = Symbol | string | number;

export enum ContentType {
  Json = "application/json",
  FormData = "multipart/form-data",
  UrlEncoded = "application/x-www-form-urlencoded",
  Text = "text/plain",
}

export class HttpClient<SecurityDataType = unknown> {
  public baseUrl: string = "";
  private securityData: SecurityDataType | null = null;
  private securityWorker?: ApiConfig<SecurityDataType>["securityWorker"];
  private abortControllers = new Map<CancelToken, AbortController>();
  private customFetch = (...fetchParams: Parameters<typeof fetch>) => fetch(...fetchParams);

  private baseApiParams: RequestParams = {
    credentials: "same-origin",
    headers: {},
    redirect: "follow",
    referrerPolicy: "no-referrer",
  };

  constructor(apiConfig: ApiConfig<SecurityDataType> = {}) {
    Object.assign(this, apiConfig);
  }

  public setSecurityData = (data: SecurityDataType | null) => {
    this.securityData = data;
  };

  protected encodeQueryParam(key: string, value: any) {
    const encodedKey = encodeURIComponent(key);
    return `${encodedKey}=${encodeURIComponent(typeof value === "number" ? value : `${value}`)}`;
  }

  protected addQueryParam(query: QueryParamsType, key: string) {
    return this.encodeQueryParam(key, query[key]);
  }

  protected addArrayQueryParam(query: QueryParamsType, key: string) {
    const value = query[key];
    return value.map((v: any) => this.encodeQueryParam(key, v)).join("&");
  }

  protected toQueryString(rawQuery?: QueryParamsType): string {
    const query = rawQuery || {};
    const keys = Object.keys(query).filter((key) => "undefined" !== typeof query[key]);
    return keys
      .map((key) => (Array.isArray(query[key]) ? this.addArrayQueryParam(query, key) : this.addQueryParam(query, key)))
      .join("&");
  }

  protected addQueryParams(rawQuery?: QueryParamsType): string {
    const queryString = this.toQueryString(rawQuery);
    return queryString ? `?${queryString}` : "";
  }

  private contentFormatters: Record<ContentType, (input: any) => any> = {
    [ContentType.Json]: (input: any) =>
      input !== null && (typeof input === "object" || typeof input === "string") ? JSON.stringify(input) : input,
    [ContentType.Text]: (input: any) => (input !== null && typeof input !== "string" ? JSON.stringify(input) : input),
    [ContentType.FormData]: (input: any) =>
      Object.keys(input || {}).reduce((formData, key) => {
        const property = input[key];
        formData.append(
          key,
          property instanceof Blob
            ? property
            : typeof property === "object" && property !== null
              ? JSON.stringify(property)
              : `${property}`,
        );
        return formData;
      }, new FormData()),
    [ContentType.UrlEncoded]: (input: any) => this.toQueryString(input),
  };

  protected mergeRequestParams(params1: RequestParams, params2?: RequestParams): RequestParams {
    return {
      ...this.baseApiParams,
      ...params1,
      ...(params2 || {}),
      headers: {
        ...(this.baseApiParams.headers || {}),
        ...(params1.headers || {}),
        ...((params2 && params2.headers) || {}),
      },
    };
  }

  protected createAbortSignal = (cancelToken: CancelToken): AbortSignal | undefined => {
    if (this.abortControllers.has(cancelToken)) {
      const abortController = this.abortControllers.get(cancelToken);
      if (abortController) {
        return abortController.signal;
      }
      return void 0;
    }

    const abortController = new AbortController();
    this.abortControllers.set(cancelToken, abortController);
    return abortController.signal;
  };

  public abortRequest = (cancelToken: CancelToken) => {
    const abortController = this.abortControllers.get(cancelToken);

    if (abortController) {
      abortController.abort();
      this.abortControllers.delete(cancelToken);
    }
  };

  public request = async <T = any, E = any>({
    body,
    secure,
    path,
    type,
    query,
    format,
    baseUrl,
    cancelToken,
    ...params
  }: FullRequestParams): Promise<HttpResponse<T, E>> => {
    const secureParams =
      ((typeof secure === "boolean" ? secure : this.baseApiParams.secure) &&
        this.securityWorker &&
        (await this.securityWorker(this.securityData))) ||
      {};
    const requestParams = this.mergeRequestParams(params, secureParams);
    const queryString = query && this.toQueryString(query);
    const payloadFormatter = this.contentFormatters[type || ContentType.Json];
    const responseFormat = format || requestParams.format;

    return this.customFetch(`${baseUrl || this.baseUrl || ""}${path}${queryString ? `?${queryString}` : ""}`, {
      ...requestParams,
      headers: {
        ...(requestParams.headers || {}),
        ...(type && type !== ContentType.FormData ? { "Content-Type": type } : {}),
      },
      signal: (cancelToken ? this.createAbortSignal(cancelToken) : requestParams.signal) || null,
      body: typeof body === "undefined" || body === null ? null : payloadFormatter(body),
    }).then(async (response) => {
      const r = response.clone() as HttpResponse<T, E>;
      r.data = null as unknown as T;
      r.error = null as unknown as E;

      const data = !responseFormat
        ? r
        : await response[responseFormat]()
            .then((data) => {
              if (r.ok) {
                r.data = data;
              } else {
                r.error = data;
              }
              return r;
            })
            .catch((e) => {
              r.error = e;
              return r;
            });

      if (cancelToken) {
        this.abortControllers.delete(cancelToken);
      }

      if (!response.ok) throw data;
      return data;
    });
  };
}

/**
 * @title ODD-BOX ADMIN-API 🤯
 * @version 0.1.7-preview2
 * @license
 * @externalDocs https://github.com/OlofBlomqvist/odd-box
 * @contact Olof Blomqvist <olof@twnet.se>
 *
 * A basic management api for odd-box reverse proxy.
 */
export class Api<SecurityDataType extends unknown> extends HttpClient<SecurityDataType> {
  api = {
    /**
     * No description
     *
     * @tags Settings
     * @name Settings
     * @summary Get global settings
     * @request GET:/api/settings
     */
    settings: (params: RequestParams = {}) =>
      this.request<SettingsData, string>({
        path: `/api/settings`,
        method: "GET",
        format: "json",
        ...params,
      }),

    /**
     * @description Note that global settings currently require a manual restart of odd-box to take effect. This will be improved in the future..
     *
     * @tags Settings
     * @name SaveSettings
     * @summary Update the global settings.
     * @request POST:/api/settings
     */
    saveSettings: (data: SaveGlobalConfig, params: RequestParams = {}) =>
      this.request<SaveSettingsData, string>({
        path: `/api/settings`,
        method: "POST",
        body: data,
        type: ContentType.Json,
        ...params,
      }),

    /**
     * No description
     *
     * @tags Site management
     * @name List
     * @summary List all configured sites.
     * @request GET:/api/sites
     */
    list: (params: RequestParams = {}) =>
      this.request<ListData, string>({
        path: `/api/sites`,
        method: "GET",
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags Site management
     * @name Set
     * @summary Update a specific item by hostname
     * @request POST:/api/sites
     */
    set: (
      data: UpdateRequest,
      query?: {
        /**
         * Optionally provide the hostname of an existing site to update
         * @example "my_site.com"
         */
        hostname?: string | null;
      },
      params: RequestParams = {},
    ) =>
      this.request<SetData, string>({
        path: `/api/sites`,
        method: "POST",
        query: query,
        body: data,
        type: ContentType.Json,
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags Site management
     * @name Delete
     * @summary Delete an item
     * @request DELETE:/api/sites
     */
    delete: (
      query: {
        /** @example "my_site.com" */
        hostname: string;
      },
      params: RequestParams = {},
    ) =>
      this.request<DeleteData, string>({
        path: `/api/sites`,
        method: "DELETE",
        query: query,
        ...params,
      }),

    /**
     * No description
     *
     * @tags Site management
     * @name Start
     * @summary Start a site
     * @request PUT:/api/sites/start
     */
    start: (
      query: {
        /** @example "my_site.com" */
        hostname: string;
      },
      params: RequestParams = {},
    ) =>
      this.request<StartData, string>({
        path: `/api/sites/start`,
        method: "PUT",
        query: query,
        ...params,
      }),

    /**
     * No description
     *
     * @tags Site management
     * @name Status
     * @summary List all configured sites.
     * @request GET:/api/sites/status
     */
    status: (params: RequestParams = {}) =>
      this.request<StatusData, string>({
        path: `/api/sites/status`,
        method: "GET",
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags Site management
     * @name Stop
     * @summary Stop a site
     * @request PUT:/api/sites/stop
     */
    stop: (
      query: {
        /** @example "my_site.com" */
        hostname: string;
      },
      params: RequestParams = {},
    ) =>
      this.request<StopData, string>({
        path: `/api/sites/stop`,
        method: "PUT",
        query: query,
        ...params,
      }),
  };
  ws = {
    /**
     * @description Warning: The format of messages emitted is not guaranteed to be stable.
     *
     * @tags Logs
     * @name LiveLogs
     * @summary Simple websocket interface for log messages.
     * @request GET:/ws/live_logs
     */
    liveLogs: (params: RequestParams = {}) =>
      this.request<any, any>({
        path: `/ws/live_logs`,
        method: "GET",
        ...params,
      }),
  };
}
