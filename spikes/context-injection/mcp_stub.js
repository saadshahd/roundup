#!/usr/bin/env node
// E7's stdio MCP server: records when it is started, offers one tool, "ping".
const fs = require("fs");
fs.appendFileSync(process.env.CI_MCP_STARTED_FILE || "/tmp/ci_mcp_started", Date.now() / 1000 + "\n");
let buf = "";
process.stdin.on("data", (d) => {
  buf += d;
  let i;
  while ((i = buf.indexOf("\n")) >= 0) {
    const line = buf.slice(0, i); buf = buf.slice(i + 1);
    let m; try { m = JSON.parse(line); } catch { continue; }
    if (m.id === undefined) continue;
    const r = m.method === "initialize"
      ? { protocolVersion: (m.params || {}).protocolVersion || "2025-06-18", capabilities: { tools: {} }, serverInfo: { name: "stub", version: "0" } }
      : m.method === "tools/list" ? { tools: [{ name: "ping", description: "returns pong", inputSchema: { type: "object", properties: {} } }] } : m.method === "tools/call" ? { content: [{ type: "text", text: "pong-from-stub" }] } : {};
    process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: m.id, result: r }) + "\n");
  }
});
