// 纯 Node 实现的 stdio MCP server（零依赖）：换行分隔的 JSON-RPC 2.0。
// 由边车在会话装配期按 manifest 的 contributes.mcpServers 拉起：
//   command: node  args: ["${CLAUDE_PLUGIN_ROOT}/server.mjs"]
// 提供两个演示工具：echo（回显）与 word_count（字符/单词/行统计）。
import readline from "node:readline";

const SERVER_INFO = { name: "echo", version: "0.1.0" };

function textResult(text) {
  return { content: [{ type: "text", text }] };
}

const tools = [
  {
    name: "echo",
    description: "将输入文本原样回显（MCP 连通性冒烟用）。",
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
  // 通知（无 id）不回复。
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
