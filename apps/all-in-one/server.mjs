// 全组合示例的 stdio MCP server（零依赖，纯 Node）：与 apps/mcp-echo/server.mjs
// 同一实现模板，serverName 为 `toolbox`（工具名 mcp__toolbox__echo / mcp__toolbox__word_count）。
import readline from "node:readline";

const SERVER_INFO = { name: "toolbox", version: "0.1.0" };

function textResult(text) {
  return { content: [{ type: "text", text }] };
}

const tools = [
  {
    name: "echo",
    description: "将输入文本原样回显（App 贡献 MCP 的连通性冒烟工具）。",
    inputSchema: {
      type: "object",
      properties: { text: { type: "string", description: "要回显的文本" } },
      required: ["text"],
    },
  },
  {
    name: "word_count",
    description: "统计输入文本的字符数 / 单词数 / 行数，返回 JSON 字符串。",
    inputSchema: {
      type: "object",
      properties: { text: { type: "string", description: "要统计的文本" } },
      required: ["text"],
    },
  },
];

function callTool(name, args) {
  const text = String(args?.text ?? "");
  switch (name) {
    case "echo":
      return textResult(text);
    case "word_count": {
      const chars = [...text].length;
      const words = text.split(/\s+/).filter(Boolean).length;
      const lines = text === "" ? 0 : text.split(/\r\n|\r|\n/).length;
      return textResult(JSON.stringify({ chars, words, lines }));
    }
    default:
      return { isError: true, content: [{ type: "text", text: `未知工具：${name}` }] };
  }
}

const rl = readline.createInterface({ input: process.stdin });
rl.on("line", (line) => {
  const trimmed = line.trim();
  if (!trimmed) return;
  let msg;
  try {
    msg = JSON.parse(trimmed);
  } catch {
    return;
  }
  if (msg.id === undefined || msg.id === null) return;

  const { method, params } = msg;
  let result;
  if (method === "initialize") {
    result = {
      protocolVersion: params?.protocolVersion ?? "2024-11-05",
      capabilities: { tools: {} },
      serverInfo: SERVER_INFO,
    };
  } else if (method === "ping") {
    result = {};
  } else if (method === "tools/list") {
    result = { tools };
  } else if (method === "tools/call") {
    result = callTool(params?.name, params?.arguments);
  } else {
    process.stdout.write(
      JSON.stringify({ jsonrpc: "2.0", id: msg.id, error: { code: -32601, message: `方法不存在：${method}` } }) + "\n",
    );
    return;
  }
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result }) + "\n");
});
