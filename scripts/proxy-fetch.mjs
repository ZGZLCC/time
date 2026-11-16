/**
 * 代理感知的 HTTPS 请求，供验证脚本使用。
 *
 * Node 的 fetch 不读 Windows 的「Internet 选项」代理设置，只认环境变量，
 * 于是直连会被 *.workers.dev 的 DNS 污染挡住。桌面版走的是 WebView2，
 * 它读系统代理，所以这里也照系统代理来，才能与桌面版行为一致。
 */
import { execFileSync } from "node:child_process";
import { request as httpsRequest } from "node:https";
import { connect as netConnect } from "node:net";
import { connect as tlsConnect } from "node:tls";

export function detectProxy() {
  const fromEnv = [process.env.HTTPS_PROXY, process.env.https_proxy, process.env.ALL_PROXY].find(
    (value) => value && value.trim() !== "",
  );
  if (fromEnv) return fromEnv.trim();
  if (process.platform !== "win32") return "";
  try {
    const key = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings";
    const output = execFileSync("reg", ["query", key], { encoding: "utf8", windowsHide: true });
    if (!/ProxyEnable\s+REG_DWORD\s+0x1/i.test(output)) return "";
    const match = /ProxyServer\s+REG_SZ\s+(\S+)/i.exec(output);
    if (!match) return "";
    const server = match[1];
    const single = server.split(";").find((part) => !part.includes("=")) ?? "";
    const chosen = (single || /https=([^;]+)/i.exec(server)?.[1] || "").trim();
    if (chosen === "") return "";
    return chosen.includes("://") ? chosen : `http://${chosen}`;
  } catch {
    return "";
  }
}

function tunnel(target, proxy) {
  return new Promise((resolve, reject) => {
    const proxyUrl = new URL(proxy);
    const port = Number(proxyUrl.port || (proxyUrl.protocol === "https:" ? 443 : 80));
    const socket = netConnect({ host: proxyUrl.hostname, port });
    let settled = false;
    const fail = (error) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      reject(error);
    };
    socket.setTimeout(15_000, () => fail(new Error(`连接代理 ${proxyUrl.host} 超时`)));
    socket.once("error", (error) => fail(new Error(`代理连接失败：${error.message}`)));
    socket.once("connect", () => {
      socket.write(
        `CONNECT ${target.host}:${target.port} HTTP/1.1\r\nHost: ${target.host}:${target.port}\r\nConnection: keep-alive\r\n\r\n`,
      );
    });
    let response = "";
    const onData = (chunk) => {
      response += chunk.toString("latin1");
      if (!response.includes("\r\n\r\n")) return;
      socket.off("data", onData);
      socket.setTimeout(0);
      const status = Number(/HTTP\/1\.[01] (\d+)/.exec(response)?.[1] ?? 0);
      if (status !== 200) return fail(new Error(`代理拒绝了 CONNECT（HTTP ${status || "未知"}）`));
      settled = true;
      resolve(socket);
    };
    socket.on("data", onData);
  });
}

/** 发一次请求，返回 { status, headers, body }；有代理时先建隧道。 */
export function sendRequest({ url, method = "GET", headers = {}, body, proxy = "" }) {
  const target = new URL(url);
  const host = target.hostname;
  const port = Number(target.port || 443);
  const options = {
    method,
    host,
    port,
    path: `${target.pathname}${target.search}`,
    headers: { Host: host, ...headers },
  };

  return new Promise((resolve, reject) => {
    const onResponse = (response) => {
      let text = "";
      response.setEncoding("utf8");
      response.on("data", (chunk) => {
        text += chunk;
      });
      response.on("end", () => resolve({ status: response.statusCode ?? 0, headers: response.headers, body: text }));
    };
    const handleError = (error) => reject(new Error(error?.code ? `${error.message}（${error.code}）` : error.message));

    if (!proxy) {
      const request = httpsRequest(options, onResponse);
      request.on("error", handleError);
      if (body !== undefined) request.write(body);
      request.end();
      return;
    }
    tunnel({ host, port }, proxy)
      .then((socket) => {
        const request = httpsRequest(
          { ...options, createConnection: () => tlsConnect({ socket, servername: host }) },
          onResponse,
        );
        request.on("error", handleError);
        if (body !== undefined) request.write(body);
        request.end();
      })
      .catch(reject);
  });
}
