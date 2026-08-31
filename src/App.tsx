import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type FolderStat = { name: string; count: number; latest_file: string; latest_mtime: number; protected: boolean };
type Overview = { total_md: number; folders: FolderStat[]; recent: { path: string; mtime: number }[] };
type TagIssue = { kind: string; tag: string; count: number; detail: string };
type Health = {
  inbox_count: number;
  broken_links: { source: string; target: string }[];
  broken_links_total: number;
  tag_issues: TagIssue[];
  naming_violations: string[];
  empty_files: string[];
};
type InboxItem = { path: string; name: string; mtime: number; preview: string };

const fmtDate = (s: number) =>
  s ? new Date(s * 1000).toLocaleString("ja-JP", { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }) : "—";

function OverviewTab() {
  const [data, setData] = useState<Overview | null>(null);
  const [err, setErr] = useState("");
  useEffect(() => {
    invoke<Overview>("vault_overview").then(setData).catch((e) => setErr(String(e)));
  }, []);
  if (err) return <p className="error">{err}</p>;
  if (!data) return <p className="muted">読み込み中…</p>;
  const max = Math.max(...data.folders.map((f) => f.count));
  return (
    <div>
      <p className="big">全ノート <b>{data.total_md.toLocaleString()}</b> 件</p>
      <h2>フォルダ別</h2>
      <table>
        <thead><tr><th>フォルダ</th><th className="num">件数</th><th>最終更新</th><th>最新ファイル</th></tr></thead>
        <tbody>
          {data.folders.map((f) => (
            <tr key={f.name}>
              <td>
                {f.name} {f.protected && <span className="badge" title="自動ツールの書き込み先。動かしたらあかん">🔒自動取込</span>}
                <div className="bar" style={{ width: `${(f.count / max) * 100}%` }} />
              </td>
              <td className="num">{f.count.toLocaleString()}</td>
              <td className="muted">{fmtDate(f.latest_mtime)}</td>
              <td className="muted small">{f.latest_file}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <h2>最近さわったノート（30件）</h2>
      <ul className="plain">
        {data.recent.map((r) => (
          <li key={r.path}><span className="muted">{fmtDate(r.mtime)}</span> {r.path}</li>
        ))}
      </ul>
    </div>
  );
}

function HealthTab() {
  const [h, setH] = useState<Health | null>(null);
  const [err, setErr] = useState("");
  const load = () => invoke<Health>("health_check").then(setH).catch((e) => setErr(String(e)));
  useEffect(() => { load(); }, []);
  if (err) return <p className="error">{err}</p>;
  if (!h) return <p className="muted">診断中…（2万ノートあるからちょっと待ってな）</p>;
  const ok = (n: number) => (n === 0 ? "ok" : "warn");
  return (
    <div>
      <div className="cards">
        <div className={`card ${h.inbox_count > 10 ? "warn" : "ok"}`}><div className="cardnum">{h.inbox_count}</div>Inbox滞留</div>
        <div className={`card ${ok(h.broken_links_total)}`}><div className="cardnum">{h.broken_links_total}</div>リンク切れ</div>
        <div className={`card ${ok(h.tag_issues.length)}`}><div className="cardnum">{h.tag_issues.length}</div>タグゆれ</div>
        <div className={`card ${ok(h.naming_violations.length)}`}><div className="cardnum">{h.naming_violations.length}</div>命名ルール違反</div>
        <div className={`card ${ok(h.empty_files.length)}`}><div className="cardnum">{h.empty_files.length}</div>空ファイル</div>
      </div>
      <button onClick={() => { setH(null); load(); }}>再診断</button>

      {h.tag_issues.length > 0 && (<>
        <h2>タグゆれ</h2>
        <table><thead><tr><th>種類</th><th>タグ</th><th className="num">件数</th><th>詳細</th></tr></thead><tbody>
          {h.tag_issues.slice(0, 50).map((t, i) => (
            <tr key={i}><td>{t.kind === "hash" ? "#付き" : t.kind === "case_collision" ? "大小文字" : "末尾_"}</td><td>{t.tag}</td><td className="num">{t.count}</td><td className="muted">{t.detail}</td></tr>
          ))}
        </tbody></table>
      </>)}

      {h.naming_violations.length > 0 && (<>
        <h2>命名ルール違反（スペース混入・日付重複）</h2>
        <ul className="plain">{h.naming_violations.slice(0, 50).map((p) => <li key={p}>{p}</li>)}</ul>
      </>)}

      {h.empty_files.length > 0 && (<>
        <h2>空ファイル</h2>
        <ul className="plain">{h.empty_files.slice(0, 50).map((p) => <li key={p}>{p}</li>)}</ul>
      </>)}

      <h2>リンク切れ（先頭100件 / 全{h.broken_links_total}件）</h2>
      <table><thead><tr><th>リンク元</th><th>切れてるリンク</th></tr></thead><tbody>
        {h.broken_links.map((b, i) => (
          <tr key={i}><td className="small">{b.source}</td><td className="small">[[{b.target}]]</td></tr>
        ))}
      </tbody></table>
    </div>
  );
}

function InboxTab() {
  const [items, setItems] = useState<InboxItem[]>([]);
  const [folders, setFolders] = useState<string[]>([]);
  const [sel, setSel] = useState<InboxItem | null>(null);
  const [body, setBody] = useState("");
  const [msg, setMsg] = useState("");
  const load = () => {
    invoke<InboxItem[]>("list_inbox").then(setItems);
    invoke<string[]>("list_dest_folders").then(setFolders);
  };
  useEffect(() => { load(); }, []);
  const open = (it: InboxItem) => {
    setSel(it);
    setBody("");
    invoke<string>("read_note", { path: it.path }).then(setBody).catch((e) => setBody(String(e)));
  };
  const move = (dest: string) => {
    if (!sel) return;
    invoke<string>("move_note", { path: sel.path, destFolder: dest })
      .then((newPath) => { setMsg(`✅ ${sel.name} → ${newPath}`); setSel(null); load(); })
      .catch((e) => setMsg(`⚠️ ${e}`));
  };
  return (
    <div className="split">
      <div className="pane">
        <h2>00_Inbox（{items.length}件）</h2>
        {items.length === 0 && <p className="muted">空っぽや！えらいで✨</p>}
        <ul className="plain click">
          {items.map((it) => (
            <li key={it.path} className={sel?.path === it.path ? "active" : ""} onClick={() => open(it)}>
              <b>{it.name}</b>
              <div className="muted small">{fmtDate(it.mtime)}</div>
            </li>
          ))}
        </ul>
        {msg && <p>{msg}</p>}
      </div>
      <div className="pane">
        {sel ? (<>
          <h2>{sel.name}</h2>
          <div className="movebar">
            移動先:
            {folders.map((f) => <button key={f} onClick={() => move(f)}>{f}</button>)}
          </div>
          <pre className="preview">{body || sel.preview}</pre>
        </>) : <p className="muted">左のノートを選ぶと中身が見えるで。移動してもファイル名は変わらんから [[リンク]] は壊れへん。移動は _meta/整理ログ_オブシリアンくん.md に記録するで。</p>}
      </div>
    </div>
  );
}

export default function App() {
  const [tab, setTab] = useState<"overview" | "health" | "inbox">("overview");
  return (
    <main>
      <header>
        <h1>🛡️ オブシリアンくん</h1>
        <nav>
          <button className={tab === "overview" ? "on" : ""} onClick={() => setTab("overview")}>俯瞰マップ</button>
          <button className={tab === "health" ? "on" : ""} onClick={() => setTab("health")}>健康診断</button>
          <button className={tab === "inbox" ? "on" : ""} onClick={() => setTab("inbox")}>Inbox振り分け</button>
        </nav>
      </header>
      {tab === "overview" && <OverviewTab />}
      {tab === "health" && <HealthTab />}
      {tab === "inbox" && <InboxTab />}
    </main>
  );
}
