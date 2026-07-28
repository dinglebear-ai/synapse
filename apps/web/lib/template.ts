import operationMetadata from "./generated-operation-metadata.json";
import { DOCKER_ACTION_PRESENTATIONS } from "./template-docker-actions";
import { SCOUT_ACTION_PRESENTATIONS } from "./template-scout-actions";

export const WEB_APP_CONFIG = {
  serviceName: "synapse",
  displayName: "Synapse",
  dashboardTitle: "Synapse Operator Dashboard",
  description: "Local Synapse workflows for Flux and Scout operations",
  apiBaseUrl: process.env.NEXT_PUBLIC_SYNAPSE_API_BASE_URL ?? "",
  restEndpoint: "/v1/synapse",
  healthEndpoint: "/health",
  statusEndpoint: "/status",
  activityEndpoint: "/activity",
  capabilitiesEndpoint: "/capabilities",
  mcpEndpoint: "/mcp",
} as const;

type ActionParamBase = {
  name: string;
  label: string;
  placeholder?: string;
  required: boolean;
  description: string;
};

export type ActionParam =
  | (ActionParamBase & {
      type: "select";
      options: readonly [string, ...string[]];
    })
  | (ActionParamBase & {
      type: "text" | "number" | "checkbox" | "string-list";
      options?: never;
    });

export type ActionScope = "synapse:read" | "synapse:write" | "public";

export type ActionSpec = {
  id: string;
  label: string;
  description: string;
  scope: ActionScope;
  destructive?: boolean;
  transport: "rest" | "mcp-only";
  params: readonly ActionParam[];
  example: {
    action: string;
    params: Record<string, unknown>;
  };
  response: Record<string, unknown>;
};

export type ActionPresentation = Omit<ActionSpec, "scope" | "destructive" | "transport">;

const ACTION_PRESENTATIONS = [
  ...DOCKER_ACTION_PRESENTATIONS,
  ...SCOUT_ACTION_PRESENTATIONS,
] as const satisfies readonly ActionPresentation[];

type RestOperationMetadata = {
  name: string;
  scope: ActionScope;
  destructive: boolean;
};

const REST_OPERATION_METADATA = new Map(
  (operationMetadata.rest_operations as RestOperationMetadata[]).map((operation) => [
    operation.name,
    operation,
  ]),
);

export type RestActionId = (typeof ACTION_PRESENTATIONS)[number]["id"];
export type RestAction = Omit<ActionSpec, "id" | "transport"> & {
  id: RestActionId;
  transport: "rest";
};

export const ACTIONS: readonly RestAction[] = ACTION_PRESENTATIONS.map((presentation) => {
  const metadata = REST_OPERATION_METADATA.get(presentation.id);
  if (!metadata) throw new Error(`Missing canonical REST metadata for ${presentation.id}`);
  return {
    ...presentation,
    scope: metadata.scope,
    destructive: metadata.destructive,
    transport: "rest",
  };
});

export const REST_ACTIONS = ACTIONS;
export const DEFAULT_REST_ACTION: RestAction = REST_ACTIONS[0];

export function normalizeApiBaseUrl(apiBaseUrl: string): string {
  return apiBaseUrl.replace(/\/+$/, "");
}

export function endpoint(path: string): string {
  return `${normalizeApiBaseUrl(WEB_APP_CONFIG.apiBaseUrl)}${path}`;
}
