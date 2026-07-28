import type { ActionPresentation } from "./template";
import { HOST_PARAM } from "./template-docker-actions";

export const SCOUT_ACTION_PRESENTATIONS = [
  {
    id: "scout.nodes",
    label: "scout.nodes",
    description: "List configured Scout nodes.",
    params: [],
    example: { action: "scout.nodes", params: {} },
    response: { nodes: [{ host: "myhost", kind: "ssh" }] },
  },
  {
    id: "scout.peek",
    label: "scout.peek",
    description: "Read a file or directory listing from a target host.",
    params: [
      { ...HOST_PARAM, required: true, description: "Host to inspect." },
      {
        name: "path",
        label: "Path",
        type: "text",
        placeholder: "/etc/hostname",
        required: true,
        description: "File or directory path to inspect.",
      },
      {
        name: "tree",
        label: "Tree",
        type: "checkbox",
        required: false,
        description: "Render directories as a tree.",
      },
      {
        name: "depth",
        label: "Depth",
        type: "number",
        placeholder: "3",
        required: false,
        description: "Tree depth, clamped by the server.",
      },
    ],
    example: { action: "scout.peek", params: { host: "myhost", path: "/etc/hostname" } },
    response: { host: "myhost", path: "/etc/hostname", content: "myhost\n" },
  },
  {
    id: "scout.exec",
    label: "scout.exec",
    description: "Run an allowlisted command on a target host.",
    params: [
      { ...HOST_PARAM, required: true, description: "Host to execute on." },
      {
        name: "path",
        label: "Working directory",
        type: "text",
        placeholder: "/tmp",
        required: false,
        description: "Optional working directory.",
      },
      {
        name: "command",
        label: "Command",
        type: "text",
        placeholder: "hostname",
        required: true,
        description: "Allowlisted command binary.",
      },
      {
        name: "args",
        label: "Args",
        type: "string-list",
        placeholder: "-la, /tmp",
        required: false,
        description: "Comma-separated positional arguments.",
      },
      {
        name: "timeout_secs",
        label: "Timeout seconds",
        type: "number",
        placeholder: "30",
        required: false,
        description: "Optional command timeout.",
      },
    ],
    example: {
      action: "scout.exec",
      params: { host: "myhost", path: "/tmp", command: "hostname" },
    },
    response: { host: "myhost", command: "hostname", stdout: "myhost\n", exit_code: 0 },
  },
] as const satisfies readonly ActionPresentation[];
