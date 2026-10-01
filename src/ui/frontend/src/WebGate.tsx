import { useEffect, useState } from "react";
import App from "./App";
import type { HostAdapter } from "./host";

export default function WebGate({ host }: { host: HostAdapter }) {
  const [authenticated, setAuthenticated] = useState<boolean | null>(null);
  const [token, setToken] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    void fetch("/api/auth", { cache: "no-store" }).then((response) => setAuthenticated(response.ok)).catch(() => { setAuthenticated(false); setError("无法连接到服务。"); });
  }, []);
  if (authenticated) return <App host={host} onUnauthenticated={() => setAuthenticated(false)} />;
  return <main className="login-screen">
    <form className="modal login-card" onSubmit={async (event) => {
      event.preventDefault(); setBusy(true); setError("");
      try {
        const response = await fetch("/api/login", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ token }) });
        if (!response.ok) throw new Error(response.status === 401 ? "访问口令不正确。" : "登录失败，请稍后重试。");
        setToken(""); setAuthenticated(true);
      } catch (reason) { setError(String(reason)); }
      finally { setBusy(false); }
    }}>
      <div className="brand-mark">g<span>·</span></div>
      <h1>geer-agent</h1>
      <p>输入服务启动时显示的访问口令，连接到共用工作区。</p>
      <label htmlFor="access-token">访问口令</label>
      <input id="access-token" type="password" autoComplete="current-password" value={token} onChange={(event) => setToken(event.target.value)} autoFocus disabled={busy || authenticated === null} />
      {error && <p role="alert" className="login-error">{error}</p>}
      <button className="primary" disabled={busy || authenticated === null || !token.trim()}>{busy ? "正在连接…" : "连接工作区"}</button>
    </form>
  </main>;
}
