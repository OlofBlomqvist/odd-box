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

export type ConfigItem =
  | {
      RemoteSite: RemoteSiteConfig;
    }
  | {
      HostedProcess: InProcessSiteConfig;
    }
  | {
      /**
       * A directory server configuration allows you to serve files from a directory on the local filesystem.
       * Both unencrypted (http) and encrypted (https) connections are supported, either self-signed or thru lets-encrypt.
       * You can specify rules for how the cache should behave, and you can also specify rules for how the files should be served.
       */
      DirServer: DirServer;
    };

export type ConfigurationItem =
  | {
      HostedProcess: InProcessSiteConfig;
    }
  | {
      RemoteSite: RemoteSiteConfig;
    }
  | {
      /**
       * A directory server configuration allows you to serve files from a directory on the local filesystem.
       * Both unencrypted (http) and encrypted (https) connections are supported, either self-signed or thru lets-encrypt.
       * You can specify rules for how the cache should behave, and you can also specify rules for how the files should be served.
       */
      DirServer: DirServer;
    };

/**
 * A directory server configuration allows you to serve files from a directory on the local filesystem.
 * Both unencrypted (http) and encrypted (https) connections are supported, either self-signed or thru lets-encrypt.
 * You can specify rules for how the cache should behave, and you can also specify rules for how the files should be served.
 */
export interface DirServer {
  /** Instead of only listening to yourdomain.com, you can capture subdomains which means this site will also respond to requests for *.yourdomain.com */
  capture_subdomains?: boolean | null;
  dir: string;
  enable_directory_browsing?: boolean | null;
  enable_lets_encrypt?: boolean | null;
  /** This is the hostname that the site will respond to. */
  host_name: string;
  redirect_to_https?: boolean | null;
  render_markdown?: boolean | null;
}

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
  env_vars?: EnvVar[] | null;
  excluded_from_start_all: boolean;
  forward_subdomains?: boolean | null;
  hints?: Hint[] | null;
  host_name: string;
  https?: boolean | null;
  log_format?: LogFormat | null;
  log_level?: LogLevel | null;
  /**
   * @format int32
   * @min 0
   */
  port?: number | null;
  proc_id: any;
  terminate_tls?: boolean | null;
}

export enum Hint {
  H2 = "H2",
  H2C = "H2C",
  H2CPK = "H2CPK",
  H1 = "H1",
  H3 = "H3",
}

export interface InProcessSiteConfig {
  /** Arguments to pass to the binary when starting it. */
  args?: string[] | null;
  /**
   * Set this to false if you do not want this site to start automatically when odd-box starts.
   * This also means that the site is excluded from the start_all command.
   */
  auto_start?: boolean | null;
  /** The binary to start. This can be a path to a binary or a command that is in the PATH. */
  bin: string;
  /** If you wish to use wildcard routing for any subdomain under the 'host_name' */
  capture_subdomains?: boolean | null;
  /** Working directory for the process. If this is not set, the current working directory will be used. */
  dir?: string | null;
  /**
   * If you want to use lets-encrypt for generating certificates automatically for this site.
   * Defaults to false. This feature will disable tcp tunnel mode.
   */
  enable_lets_encrypt?: boolean | null;
  /** Environment variables to set for the process. */
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
  /**
   * H1,H2,H2C,H2CPK,H3 - empty means H1 is expected to work with passthru: everything else will be
   * using terminating mode.
   */
  hints?: Hint[] | null;
  host_name: string;
  https?: boolean | null;
  /** Defaults to true. */
  keep_original_host_header?: boolean | null;
  log_format?: LogFormat | null;
  log_level?: LogLevel | null;
  /**
   * If this is set to None, the next available port will be used. Starting from the global port_range_start
   * @format int32
   * @min 0
   */
  port?: number | null;
  terminate_http?: boolean | null;
  /**
   * This is mostly useful in case the target uses SNI sniffing/routing.
   * It means you want to use level 7 mode instead of level 4, thus always terminating connections.
   * Previously this setting was called 'disable_tcp_tunnel_mode'
   */
  terminate_tls?: boolean | null;
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
  odd_box_password: string;
  odd_box_url: string;
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
  V3 = "V3",
}

export interface OddBoxV3Config {
  /** Defaults to true. Lets you enable/disable h2/http11 tls alpn algs during initial connection phase. */
  alpn?: boolean | null;
  /**
   * If this is set to false, odd-box will not start any hosted processes automatically when it starts
   * unless they are set to auto_start individually. Same with true, it will start all processes that
   * have not been specifically configured with auto_start=false.
   */
  auto_start?: boolean | null;
  default_log_format?: LogFormat;
  /** Used for static websites. */
  dir_server?: DirServer[] | null;
  /** Environment variables configured here will be made available to all processes started by odd-box. */
  env_vars?: EnvVar[];
  /**
   * Used to set up processes to keep running and serve requests on a specific hostname.
   * This can be used to run a web server, a proxy, or any other kind of process that can handle http requests.
   * It can also be used even if the process is not a web server and you just want to keep it running..
   */
  hosted_process?: InProcessSiteConfig[] | null;
  /**
   * The port on which to listen for http requests. Defaults to 8080.
   * @format int32
   * @min 0
   */
  http_port?: number | null;
  ip: string;
  /** If you want to use lets-encrypt for generating certificates automatically for your sites */
  lets_encrypt_account_email?: string | null;
  log_level?: LogLevel | null;
  /** Used for securing the admin api and web-interface. If you do not set this, anyone can access the admin api. */
  odd_box_password?: string | null;
  /**
   * If you want to use a specific odd-box url for the admin api and web-interface you can
   * configure the host_name to listen on here. This is useful if you want to use a specific domain
   * for the admin interface and the api. If you do not set this, the admin interface will be available
   * on https://localhost and https://odd-box.localhost by default.
   * If you configure this, you should also configure the odd_box_password property.
   */
  odd_box_url?: string | null;
  /**
   * The port range start is used to determine which ports to use for hosted processes.
   * @format int32
   * @min 0
   */
  port_range_start?: number;
  /** Used to configure remote (or local sites not managed by odd-box) as a targets for requests. */
  remote_target?: RemoteSiteConfig[] | null;
  /**
   * Optionally configure the $root_dir variable which you can use in environment variables, paths and other settings.
   * By default $root_dir will be $pwd (dir where odd-box is started).
   */
  root_dir?: string | null;
  /**
   * The port on which to listen for https requests. Defaults to 4343.
   * @format int32
   * @min 0
   */
  tls_port?: number | null;
  /** Uses 127.0.0.1 instead of localhost when proxying to locally hosted processes. */
  use_loopback_ip_for_procs?: boolean | null;
  /** The schema version - you do not normally need to set this, it is set automatically when you save the configuration. */
  version: string;
}

export enum ProcState {
  Faulty = "Faulty",
  Stopped = "Stopped",
  Starting = "Starting",
  Stopping = "Stopping",
  Running = "Running",
  Remote = "Remote",
  DirServer = "DirServer",
  Docker = "Docker",
}

export interface RemoteSiteConfig {
  backends: Backend[];
  /** If you wish to use wildcard routing for any subdomain under the 'host_name' */
  capture_subdomains?: boolean | null;
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
  /**
   * If you wish to pass along the incoming request host header to the backend
   * rather than the host name of the backends. Defaults to false.
   */
  keep_original_host_header?: boolean | null;
  /**
   * Enforce the termination of http requests.
   * Only enable if you know exactly why you want this as it can negatively affect performance.
   */
  terminate_http?: boolean | null;
  /**
   * This is mostly useful in case the target uses SNI sniffing/routing.
   * It means you want to use level 7 mode instead of level 4, thus always terminating connections.
   * Previously this setting was called 'disable_tcp_tunnel_mode'
   */
  terminate_tls?: boolean | null;
}

export interface ReqRule {
  /** If no index.html is found, you can set this to true to allow directory browsing. */
  allow_directory_browsing?: boolean | null;
  /**
   * The max age in seconds for the cache. If this is set to None, the cache will be disabled.
   * This setting causes odd-box to add a Cache-Control header to the response.
   * @format int64
   * @min 0
   */
  max_age_in_seconds?: number | null;
  /**
   * Full url path of the file this rule should apply to, or a regex pattern for the url.
   * For example: /index.html or /.*\.html
   */
  path_pattern?: string | null;
}

export interface SaveGlobalConfig {
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
  odd_box_password: string;
  odd_box_url: string;
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

export interface SiteStatusEvent {
  host_name: string;
  id: any;
  state: State;
}

export type SitesError = {
  UnknownError: string;
};

export enum State {
  Faulty = "Faulty",
  Stopped = "Stopped",
  Starting = "Starting",
  Stopping = "Stopping",
  Running = "Running",
  Remote = "Remote",
  DirServer = "DirServer",
  Docker = "Docker",
}

export interface StatusItem {
  hostname: string;
  state: any;
}

export interface StatusResponse {
  items: StatusItem[];
}

export interface UpdateRequest {
  new_configuration: ConfigItem;
}

export enum V3VersionEnum {
  V3 = "V3",
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
 * @version 0.1.11-alpha2
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
     * @tags Events
     * @name EventStream
     * @summary Simple websocket interface for log messages.
     * @request GET:/ws/event_stream
     */
    eventStream: (params: RequestParams = {}) =>
      this.request<any, any>({
        path: `/ws/event_stream`,
        method: "GET",
        ...params,
      }),
  };
}
