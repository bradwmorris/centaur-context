import { useState } from "react";
import { sourceThumbnailUrl, type Source, type SourceViewProps } from "@centaur-context/ui";
import styles from "./styles.module.css";

const label = (kind: string) => kind.replaceAll("_", " ");
function Preview({ source, refreshKey }: { source: Readonly<Source>; refreshKey: number }) {
  const [failed, setFailed] = useState(false);
  return <div className={styles.preview}>
    {source.canonical_uri && !failed ? <img src={sourceThumbnailUrl(source, refreshKey)} alt="" loading="lazy" decoding="async" referrerPolicy="no-referrer" onError={() => setFailed(true)} /> : <span className={styles.fallback}><span aria-hidden="true">{source.source_kind === "video" ? "▷" : "▤"}</span><span>Preview unavailable</span></span>}
  </div>;
}
function Card({ source, refreshKey, open }: { source: Readonly<Source>; refreshKey: number; open(id: string): void }) {
  return <article className={styles.card}>
    <button className={styles.open} onClick={() => open(source.object_id)} aria-label={`Open ${source.title}`}>
      <Preview key={`${source.object_id}:${source.revision}:${refreshKey}`} source={source} refreshKey={refreshKey} />
      <div className={styles.content}>
        <span className={styles.kind}>{label(source.source_kind)}</span>
        <h3>{source.title}</h3>
        <p className={styles.description}>{source.description}</p>
        <p className={styles.byline}>{source.publisher || source.byline || "Source"}{source.publisher && source.byline ? ` · ${source.byline}` : ""}</p>
      </div>
    </button>
  </article>;
}
export default function SourceCollection({ layout, sources, loading, error, completeness, reload, openSource }: SourceViewProps & { layout: "grid" | "board" }) {
  const [refreshKey, setRefreshKey] = useState(0);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const groups = [...new Set(sources.map(source => source.source_kind))].sort();
  return <section className={styles.collection} aria-label={`Sources ${layout === "grid" ? "grid" : "Kanban"}`}>
    <div className={styles.toolbar}><div><h2>{layout === "grid" ? "Source library" : "Sources by type"}</h2><p>{sources.length} sources{completeness !== "complete" ? " · Results may be incomplete" : ""}</p></div>
      <button onClick={async () => { setRefreshKey(Date.now()); setRefreshError(null); try { await reload(); } catch (e) { setRefreshError(e instanceof Error ? e.message : "Refresh failed"); } }}>Refresh previews</button>
    </div>
    {(error || refreshError) && <p role="alert">{error || refreshError}</p>}
    {loading && <p role="status">Loading sources…</p>}
    {!loading && sources.length === 0 && <p className={styles.empty}>No sources match this view.</p>}
    {layout === "grid" ? <div className={styles.grid}>{sources.map(source => <Card key={source.object_id} source={source} refreshKey={refreshKey} open={openSource} />)}</div> : <div className={styles.board}>{groups.map(kind => <section className={styles.column} aria-label={`${label(kind)} sources`} key={kind}><header><h3>{label(kind)}</h3><span>{sources.filter(s => s.source_kind === kind).length}</span></header><div>{sources.filter(s => s.source_kind === kind).map(source => <Card key={source.object_id} source={source} refreshKey={refreshKey} open={openSource} />)}</div></section>)}</div>}
  </section>;
}
