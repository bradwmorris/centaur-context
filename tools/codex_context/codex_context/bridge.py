"""Reviewed host-side capture; model-facing MCP has no capture authority."""
from __future__ import annotations

import argparse
import contextlib
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import sqlite3
import stat
import subprocess
import sys
import time
from typing import Any
from urllib.parse import urlparse
from urllib.request import build_opener, ProxyHandler, HTTPRedirectHandler
from urllib.error import HTTPError, URLError
import uuid

from centaur_tool_centaur_context.client import CentaurContextClient, _UrllibResponse
from . import outcomes, contract
from .privacy import redact

MAX_BATCH_BYTES = 400_000
MAX_QUEUE_BYTES = 32 * 1024 * 1024
MAX_TRANSCRIPT_READ = 16 * 1024 * 1024
MAX_LINE_BYTES = 4 * 1024 * 1024
MAX_MESSAGE_CHARS = 20_000
MAX_PENDING_BATCHES = 500
CAPTURE_EVENTS = {"SessionStart", "UserPromptSubmit", "Stop", "Interrupt", "SessionEnd"}


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def identifier(value: Any) -> str:
    return str(uuid.UUID(str(value)))


def private_file(path: Path) -> bytes:
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise ValueError("Credential file must be a regular, owner-only file")
    value = path.read_bytes().strip()
    if len(value) < 32 or len(value) > 4096:
        raise ValueError("Invalid credential file length")
    return value


def git_directory(path: Path) -> str:
    result = subprocess.run(
        ["git", "-C", str(path), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True, text=True, timeout=5, check=True,
    )
    return str(Path(result.stdout.strip()).resolve())


class UnmappedRepository(ValueError):
    """A new session is outside this explicitly opt-in installation."""


class Settings:
    def __init__(self, path: Path):
        self.path = path.resolve()
        self.data = json.loads(self.path.read_text())
        if self.data.get("version") != 1:
            raise ValueError("Unsupported bridge configuration version")
        self.host = identifier(self.data["host_id"])
        self.root = Path(self.data["state_dir"]).expanduser().resolve()
        self.root.mkdir(mode=0o700, parents=True, exist_ok=True)
        info = self.root.stat()
        if info.st_uid != os.getuid() or info.st_mode & 0o077:
            raise ValueError("Bridge state directory must be owner-only (0700)")
        self.transcript_root = Path(self.data["transcript_root"]).expanduser().resolve()
        self.versions = self.data.get("supported_versions", ["0.153.4"])
        self.targets = self.data["targets"]
        self.repositories = self.data["repositories"]
        if not self.repositories or not self.targets:
            raise ValueError("Configure explicit repository and target bindings")
        for alias, repo in self.repositories.items():
            if not alias or len(alias) > 80 or not all(c.isascii() and (c.isalnum() or c in "-_") for c in alias):
                raise ValueError("Invalid repository alias")
            if repo["target"] not in self.targets:
                raise ValueError("Unknown repository target")
        for target in self.targets.values():
            u = urlparse(target["url"])
            if u.username or u.password or u.query or u.fragment or u.path not in ("", "/"):
                raise ValueError("Context URL must be an origin without credentials")
            try:
                local = ipaddress.ip_address(u.hostname or "").is_loopback
            except ValueError:
                local = u.hostname == "localhost"
            if u.scheme != "https" and not (u.scheme == "http" and local):
                raise ValueError("Use HTTPS or an explicit loopback HTTP tunnel")

    def route(self, cwd: str, session: str) -> tuple[str, str]:
        try:
            common = git_directory(Path(cwd))
        except subprocess.CalledProcessError as error:
            raise UnmappedRepository("Capture is unconfigured for this directory") from error
        selected = self.data.get("session_bindings", {}).get(session)
        candidates = []
        for alias, repo in self.repositories.items():
            if git_directory(Path(repo["path"]).expanduser()) == common:
                candidates.append(alias)
        if selected is not None:
            if selected not in candidates:
                raise ValueError("Explicit session binding does not match its repository")
            candidates = [selected]
        if not candidates:
            raise UnmappedRepository("Capture is unconfigured for this repository")
        if len(candidates) != 1:
            raise ValueError("Capture is unconfigured: select exactly one Context destination")
        alias = candidates[0]
        return alias, self.repositories[alias]["target"]


SCHEMA = """
CREATE TABLE IF NOT EXISTS sessions (
 id TEXT PRIMARY KEY, repository TEXT NOT NULL, target TEXT NOT NULL,
 transcript TEXT, cursor INTEGER NOT NULL DEFAULT 0, transcript_identity TEXT,
 cwd TEXT NOT NULL,
 title TEXT NOT NULL, coverage TEXT NOT NULL, error TEXT, chat_id TEXT,
 updated REAL NOT NULL, started REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS batches (
 id TEXT PRIMARY KEY, session_id TEXT NOT NULL REFERENCES sessions(id),
 payload TEXT NOT NULL, created REAL NOT NULL, attempts INTEGER NOT NULL DEFAULT 0,
 next_attempt REAL NOT NULL DEFAULT 0, error TEXT
);
CREATE TABLE IF NOT EXISTS git_checkpoints (
 session_id TEXT NOT NULL, turn_id TEXT NOT NULL, cwd TEXT NOT NULL,
 head TEXT NOT NULL, started REAL NOT NULL, observed TEXT, PRIMARY KEY(session_id,turn_id)
);
CREATE TABLE IF NOT EXISTS receipts (
 batch_id TEXT PRIMARY KEY, session_id TEXT NOT NULL,
 response TEXT NOT NULL, completed REAL NOT NULL
);
"""


@contextlib.contextmanager
def database(settings: Settings):
    os.umask(0o077)
    db = sqlite3.connect(settings.root / "delivery.sqlite3", timeout=10)
    db.row_factory = sqlite3.Row
    db.execute("PRAGMA foreign_keys=ON")
    db.execute("PRAGMA journal_mode=WAL")
    db.executescript(SCHEMA)
    try:
        yield db
    finally:
        db.close()


def transcript_path(settings: Settings, raw: Any) -> Path | None:
    if raw is None:
        return None
    path = Path(raw).resolve(strict=True)
    if not path.is_relative_to(settings.transcript_root) or not path.is_file():
        raise ValueError("Transcript is outside the configured Codex transcript root")
    return path


def runtime_metadata(settings: Settings, path: Path, session: str) -> None:
    with path.open("rb") as f:
        line = f.readline(MAX_LINE_BYTES + 1)
    if len(line) > MAX_LINE_BYTES:
        raise ValueError("Oversized transcript metadata")
    item = json.loads(line)
    payload = item.get("payload", {})
    if item.get("type") != "session_meta" or payload.get("cli_version") not in settings.versions:
        raise ValueError("Unsupported Codex transcript/runtime version")
    if identifier(payload.get("id") or payload.get("session_id")) != session:
        raise ValueError("Transcript session identity mismatch")
    if payload.get("history_mode") != "paginated":
        raise ValueError("Unsupported transcript history mode; paginated history required")


def extract_messages(settings: Settings, path: Path, session: str, cursor: int, identity: str) -> tuple[list[dict], int, str, str | None]:
    runtime_metadata(settings, path, session)
    info = path.stat()
    if identity != f"{info.st_dev}:{info.st_ino}" or info.st_size < cursor:
        raise ValueError("Transcript replaced or truncated; capture requires explicit reconciliation")
    messages = []
    coverage = "text_messages"
    completed = None
    with path.open("rb") as stream:
        stream.seek(cursor)
        total = 0
        while total < MAX_TRANSCRIPT_READ:
            before = stream.tell()
            line = stream.readline(MAX_LINE_BYTES + 1)
            if len(line) > MAX_LINE_BYTES:
                raise ValueError("Oversized transcript item; capture cursor preserved")
            if not line or not line.endswith(b"\n"):
                break  # A partial write remains pending, never advance over it.
            total += len(line)
            record = json.loads(line)
            data = record.get("payload", {})
            if record.get("type") == "event_msg" and data.get("type") == "item_completed":
                item = data.get("item", {})
                visible = item.get("type") == "UserMessage" or (item.get("type") == "AgentMessage" and item.get("phase") in (None,"commentary","final","final_answer"))
                if data.get("thread_id") == session and visible:
                    pieces = item.get("content", [])
                    if not isinstance(pieces, list):
                        raise ValueError("Unsupported visible-message format")
                    texts = []
                    for piece in pieces:
                        if piece.get("type") in ("text", "Text") and isinstance(piece.get("text"), str):
                            texts.append(piece["text"])
                        else:
                            coverage = "text_only_attachments_omitted"
                    text = redact("\n".join(texts))
                    if text.strip():
                        if len(text) > MAX_MESSAGE_CHARS:
                            raise ValueError("Visible message exceeds the Context limit; not silently truncated")
                        message = {"id":str(item["id"]), "turn_id":identifier(data["turn_id"]),
                            "role":"human" if item["type"] == "UserMessage" else "agent",
                            "content":text, "created_at":record["timestamp"]}
                        if len(messages) >= 100 or len(canonical(messages + [message]).encode()) > MAX_BATCH_BYTES:
                            return messages, before, coverage, completed
                        messages.append(message)
            if record.get("type") == "event_msg" and data.get("type") == "task_complete":
                completed = identifier(data["turn_id"])
            cursor = stream.tell()
    return messages, cursor, coverage, completed


def queue_batch(db: sqlite3.Connection, session: str, payload: dict) -> None:
    size, count = db.execute("SELECT COALESCE(sum(length(CAST(payload AS BLOB))),0),count(*) FROM batches").fetchone()
    body = canonical(payload)
    if size + len(body.encode()) > MAX_QUEUE_BYTES or count >= MAX_PENDING_BATCHES:
        raise ValueError("Delivery queue is full; cursor retained and capture needs attention")
    db.execute("INSERT OR IGNORE INTO batches(id,session_id,payload,created) VALUES(?,?,?,?)",
        (payload["batch_id"],session,body,time.time()))


def capture(settings: Settings, event: dict) -> dict:
    name = event.get("hook_event_name")
    if name not in CAPTURE_EVENTS:
        raise ValueError("Unsupported capture event")
    session = identifier(event["session_id"])
    alias, target = settings.route(event["cwd"], session)
    path = transcript_path(settings, event.get("transcript_path"))
    now = time.time()
    with database(settings) as db, db:
        db.execute("BEGIN IMMEDIATE")
        prior = db.execute("SELECT * FROM sessions WHERE id=?", (session,)).fetchone()
        if prior and (prior["repository"] != alias or prior["target"] != target):
            raise ValueError("Session destination is immutable; cannot switch Context instances")
        if not prior:
            cursor, inode = 0, None
            if path:
                runtime_metadata(settings,path,session)
                info = path.stat()
                cursor, inode = info.st_size, f"{info.st_dev}:{info.st_ino}"
            db.execute("INSERT INTO sessions(id,repository,target,transcript,cursor,transcript_identity,title,coverage,updated,started,cwd) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
                (session,alias,target,str(path) if path else None,cursor,inode,
                 f"Codex: {alias}","capture_from_activation",now,now,event["cwd"]))
            queue_batch(db,session,{"version":1,"batch_id":str(uuid.uuid5(uuid.UUID(session),"bootstrap")),
                "title":f"Codex: {alias}","messages":[],"finished_turn_id":None,"coverage":"registered" if path else "unavailable"})
            prior = db.execute("SELECT * FROM sessions WHERE id=?", (session,)).fetchone()
        if path and prior["transcript"] is None:
            runtime_metadata(settings,path,session)
            info = path.stat()
            # SessionStart may have no transcript. Start at the first metadata line,
            # whose session creation time must be newer than our local binding.
            with path.open("rb") as f:
                meta_line = f.readline(MAX_LINE_BYTES + 1)
            from datetime import datetime
            meta = json.loads(meta_line)["payload"]
            created = datetime.fromisoformat(meta["timestamp"].replace("Z","+00:00")).timestamp()
            if created < prior["started"] - 10:
                raise ValueError("Cannot attach an older transcript without history review")
            db.execute("UPDATE sessions SET transcript=?,cursor=?,transcript_identity=? WHERE id=?",
                (str(path),len(meta_line),f"{info.st_dev}:{info.st_ino}",session))
            prior = db.execute("SELECT * FROM sessions WHERE id=?", (session,)).fetchone()
        if path and str(path) != prior["transcript"]:
            raise ValueError("Transcript path changed; explicit reconciliation required")
        if name == "UserPromptSubmit" and event.get("turn_id"):
            outcomes.checkpoint(db,session,identifier(event["turn_id"]),event["cwd"])
        if name == "Stop" and event.get("turn_id"):
            receipt = outcomes.observe(db,session,identifier(event["turn_id"]),event["cwd"])
            if receipt:
                queue_batch(db,session,{"version":1,"batch_id":str(uuid.uuid5(uuid.UUID(session),"git:"+receipt["commits"][-1]["oid"])),
                    "title":prior["title"],"messages":[],"finished_turn_id":None,"git_receipts":[receipt]})
        if not path:
            if prior["coverage"] != "unavailable":
                queue_batch(db,session,{"version":1,"batch_id":str(uuid.uuid5(uuid.UUID(session),f"unavailable:{prior['cursor']}")),
                    "title":prior["title"],"messages":[],"finished_turn_id":None,"coverage":"unavailable"})
            db.execute("UPDATE sessions SET coverage='unavailable',error='No transcript: full Chat capture unavailable',updated=? WHERE id=?", (now,session))
            return {"bound":alias,"coverage":"unavailable"}
        messages, cursor, coverage, finish = extract_messages(settings,path,session,prior["cursor"],prior["transcript_identity"])
        # task_complete follows the Stop hook. The next drain sees it, avoiding
        # premature curation before the final visible message is durably appended.
        if messages or finish:
            key = f"{prior['cursor']}:{cursor}:{finish or ''}"
            queue_batch(db,session,{"version":1,"batch_id":str(uuid.uuid5(uuid.UUID(session),key)),
                "title":prior["title"],"messages":messages,"finished_turn_id":finish,"coverage":coverage})
        db.execute("UPDATE sessions SET cursor=?,coverage=?,error=NULL,updated=? WHERE id=?",(cursor,coverage,now,session))
    return {"bound":alias,"coverage":coverage}


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        raise ValueError("Context redirects are forbidden")


class SafeTransport:
    def __init__(self, timeout: float = 5):
        self.timeout = timeout
        self.opener = build_opener(ProxyHandler({}),NoRedirect())

    def request(self, method, url, *, json=None, headers=None, **kwargs):
        from urllib.request import Request
        body = canonical(json).encode() if json is not None else None
        request = Request(url,data=body,headers={**(headers or {}),"Content-Type":"application/json"},method=method)
        try:
            with self.opener.open(request,timeout=self.timeout) as response:
                content = response.read(4*1024*1024+1)
                if len(content)>4*1024*1024: raise ValueError("Oversized Context response")
                return _UrllibResponse(response.status,content)
        except HTTPError as error:
            # Error text is server-controlled and can echo private inputs. Report code only.
            return _UrllibResponse(error.code,canonical({"error":{"message":f"Context HTTP {error.code}"}}).encode())
        except URLError as error:
            raise RuntimeError("Context is unavailable; delivery remains queued") from error

    def close(self):
        pass


class SessionClient(CentaurContextClient):
    def __init__(self, settings: Settings, session: sqlite3.Row, capture_token: bool = False):
        self.session = session
        target = settings.targets[session["target"]]
        token = private_file(Path(target["capture_token_file" if capture_token else "tool_token_file"]).expanduser()).decode()
        super().__init__(base_url=target["url"],token=token,principal_id="codex-bridge",thread_key="codex-bridge",
            timeout=5)
        # The core client's `transport` argument is an httpx BaseTransport;
        # install our request client directly to retain stdlib-only operation.
        self._http = SafeTransport()

    def _headers(self, idempotency_key=None, **kwargs):
        return {**super()._headers(idempotency_key,**kwargs),
            "X-Codex-Session-Id":self.session["id"],"X-Codex-Repository":self.session["repository"]}


def flush(settings: Settings, session_id: str | None = None, limit: int = 20) -> dict:
    import fcntl
    lock = (settings.root / "delivery.lock").open("a")
    try:
        try:
            fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        except BlockingIOError:
            return {"busy":True}
        delivered = 0
        # Catch up only previously registered sessions, never discover/import history.
        with database(settings) as db:
            registered = db.execute("SELECT * FROM sessions WHERE (? IS NULL OR id=?) ORDER BY updated LIMIT ?",(session_id,session_id,limit)).fetchall()
        for row in registered:
            if row["transcript"]:
                try:
                    capture(settings,{"hook_event_name":"SessionEnd","session_id":row["id"],"cwd":row["cwd"],"transcript_path":row["transcript"]})
                except (RuntimeError,ValueError,OSError,KeyError,subprocess.SubprocessError) as error:
                    with database(settings) as db,db:
                        db.execute("UPDATE sessions SET coverage='unavailable',error=?,updated=? WHERE id=?",(str(error),time.time(),row["id"]))
                        if row["coverage"] != "unavailable":
                            queue_batch(db,row["id"],{"version":1,"batch_id":str(uuid.uuid5(uuid.UUID(row["id"]),f"unavailable:{row['cursor']}")),
                                "title":row["title"],"messages":[],"finished_turn_id":None,"coverage":"unavailable"})
        with database(settings) as db:
            rows = db.execute("SELECT b.* FROM batches b WHERE (? IS NULL OR session_id=?) AND next_attempt<=? AND NOT EXISTS(SELECT 1 FROM batches older WHERE older.session_id=b.session_id AND older.rowid<b.rowid) ORDER BY rowid LIMIT ?",
                (session_id,session_id,time.time(),limit)).fetchall()
            # One batch per session each sweep preserves chronology across failure/retry.
            for batch in rows:
                session = db.execute("SELECT * FROM sessions WHERE id=?", (batch["session_id"],)).fetchone()
                try:
                    client = SessionClient(settings,session,capture_token=True)
                    response = client._request("POST","/api/v2/codex/capture",json=json.loads(batch["payload"]))
                    with db:
                        db.execute("INSERT OR REPLACE INTO receipts VALUES(?,?,?,?)",(batch["id"],session["id"],canonical(response),time.time()))
                        db.execute("UPDATE sessions SET chat_id=?,updated=? WHERE id=?",(response["chat_object_id"],time.time(),session["id"]))
                        db.execute("DELETE FROM batches WHERE id=?",(batch["id"],))
                    delivered += 1
                except (RuntimeError,ValueError,KeyError,OSError) as error:
                    attempts = batch["attempts"]+1
                    safe = type(error).__name__+": delivery failed; inspect endpoint, credentials and schema"
                    with db:
                        db.execute("UPDATE batches SET attempts=?,next_attempt=?,error=? WHERE id=?",
                            (attempts,time.time()+min(300,2**min(attempts,8)),safe,batch["id"]))
            with db:
                db.execute("DELETE FROM receipts WHERE completed<?",(time.time()-30*86400,))
                db.execute("DELETE FROM git_checkpoints WHERE started<?",(time.time()-7*86400,))
        return {"delivered":delivered}
    finally:
        lock.close()


def status(settings: Settings, session_id: str | None = None) -> dict:
    with database(settings) as db:
        sessions = []
        for row in db.execute("SELECT id,repository,target,coverage,error,chat_id,updated FROM sessions ORDER BY updated DESC"):
            item = dict(row)
            item["pending_batches"] = db.execute("SELECT count(*) FROM batches WHERE session_id=?",(row["id"],)).fetchone()[0]
            failed = db.execute("SELECT error FROM batches WHERE session_id=? AND error IS NOT NULL ORDER BY rowid LIMIT 1",(row["id"],)).fetchone()
            item["delivery_error"] = failed[0] if failed else None
            last = db.execute("SELECT response FROM receipts WHERE session_id=? ORDER BY completed DESC LIMIT 1",(row["id"],)).fetchone()
            item["last_receipt"] = json.loads(last[0]) if last else None
            sessions.append(item)
        result = {"sessions":sessions,"pending_bytes":db.execute("SELECT COALESCE(sum(length(CAST(payload AS BLOB))),0) FROM batches").fetchone()[0]}
        if session_id:
            row = db.execute("SELECT * FROM sessions WHERE id=?",(identifier(session_id),)).fetchone()
            if row is None:raise ValueError("Unknown registered session")
            try:result["remote_curation"] = SessionClient(settings,row)._request("GET","/api/v2/codex/session")
            except RuntimeError:result["remote_curation"] = {"status":"unavailable","note":"Cannot verify remote Chat or Memory curation; inspect connectivity and retry."}
        else:
            result["remote_curation"] = {"status":"not_checked","note":"Use status --session SESSION_UUID to inspect the latest actual curation Run."}
        return result


def tool_schema() -> list[dict]:
    search, read, apply = contract.schemas()
    return [
        {"name":"context_search","description":"Find canonical Objects in this session's bound Context. Use before work to retrieve relevant prior knowledge.","inputSchema":search,"annotations":{"readOnlyHint":True}},
        {"name":"context_read","description":"Read full Objects and optional messages, evidence, or Connections in this session's Context.","inputSchema":read,"annotations":{"readOnlyHint":True}},
        {"name":"context_apply","description":"Apply explicitly user-requested changes atomically. Do not turn brainstorming into records. Ordinary Tasks, Notes, Entities, Sources and Themes only; Chats, Users and Memories are system-managed. New Tasks require active owner and due date; code Tasks require a GitHub Issue. Descriptions are current snapshots <=600 characters. Use stable idempotency keys and expected_revision for updates. create_object uses local_ref,kind,title,description,fields; update_object uses object_id,expected_revision,changes; connections use source/target as {object_id} or {local_ref}. contract_version is 1.1.0. Provenance Chat is supplied by the bridge.","inputSchema":apply,"annotations":{"readOnlyHint":False,"idempotentHint":True}},
    ]


def mcp_call(settings: Settings, params: dict) -> dict:
    meta = params.get("_meta",{})
    session_id = identifier(meta.get("threadId"))
    for _ in range(3):
        if not flush(settings,session_id).get("delivered"):
            break
    with database(settings) as db:
        session = db.execute("SELECT * FROM sessions WHERE id=?",(session_id,)).fetchone()
    if session is None:
        raise ValueError("This Codex session has no trusted hook-created Context binding")
    client = SessionClient(settings,session)
    if client.context_contract() != contract.document():
        raise ValueError("Context contract differs from this installed bridge; update the bridge before using tools")
    args = params.get("arguments",{})
    name = params.get("name")
    if name == "context_search":
        result = client.context_search(**args)
    elif name == "context_read":
        result = client.context_read(**args)
    elif name == "context_apply":
        if not session["chat_id"]:
            raise ValueError("Chat delivery is pending; writes wait for provenance")
        if "chat_object_id" in args:
            raise ValueError("The bridge supplies this session's provenance")
        result = client.context_apply({**args,"chat_object_id":session["chat_id"]})
    else:
        raise ValueError("Only context_search, context_read, and context_apply are exposed")
    return {"content":[{"type":"text","text":canonical(result)}]}


def serve(settings: Settings, incoming=sys.stdin, outgoing=sys.stdout) -> None:
    for line in incoming:
        message = None
        try:
            if len(line)>512*1024: raise ValueError("Oversized MCP request")
            message = json.loads(line)
            if "id" not in message: continue
            method = message.get("method")
            if method == "initialize":
                result = {"protocolVersion":"2025-06-18","capabilities":{"tools":{}},
                    "serverInfo":{"name":"centaur-context","version":"0.1.0"},
                    "instructions":contract.guidance()}
            elif method == "tools/list": result = {"tools":tool_schema()}
            elif method == "tools/call":
                try: result=mcp_call(settings,message.get("params",{}))
                except (ValueError,RuntimeError,OSError,KeyError,TypeError) as error:
                    result={"isError":True,"content":[{"type":"text","text":str(error)}]}
            elif method == "ping": result = {}
            else:
                outgoing.write(canonical({"jsonrpc":"2.0","id":message["id"],"error":{"code":-32601,"message":"Method not supported"}})+"\n");outgoing.flush();continue
            outgoing.write(canonical({"jsonrpc":"2.0","id":message["id"],"result":result})+"\n")
        except (ValueError,KeyError,TypeError):
            outgoing.write(canonical({"jsonrpc":"2.0","id":message.get("id") if isinstance(message,dict) else None,"error":{"code":-32600,"message":"Invalid MCP request"}})+"\n")
        outgoing.flush()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config",type=Path,required=True)
    parser.add_argument("command",choices=["hook","flush","status","mcp","install","uninstall"])
    parser.add_argument("--codex-home",type=Path,default=Path.home()/".codex")
    parser.add_argument("--launchd",action="store_true")
    parser.add_argument("--session",type=identifier)
    args=parser.parse_args()
    try:
        settings=Settings(args.config)
        if args.command in ("install","uninstall"):
            from .install import install
            print(canonical(install(settings,args.codex_home,remove=args.command=="uninstall",launchd=args.launchd)));return
        if args.command=="mcp": serve(settings);return
        if args.command=="hook":
            event=json.loads(sys.stdin.read(512*1024))
            try:
                result=capture(settings,event)
            except (ValueError,RuntimeError,OSError,KeyError,subprocess.SubprocessError) as error:
                # A global hook must leave unrelated projects alone. A previously
                # bound session losing its route is still an actionable error.
                if isinstance(error,UnmappedRepository):
                    with database(settings) as db:
                        known=db.execute("SELECT 1 FROM sessions WHERE id=?",(event.get("session_id"),)).fetchone()
                    if not known:
                        print("{}");return
                # Capture errors must be visible without preventing a Codex response.
                with database(settings) as db,db:
                    if event.get("session_id"):
                        db.execute("UPDATE sessions SET error=?,updated=? WHERE id=?",(str(error),time.time(),event["session_id"]))
                print(canonical({"systemMessage":"Centaur Context capture needs attention: "+str(error)}));return
            print("{}")
            # Actual delivery runs in a separate process, preserving response latency.
            subprocess.Popen([sys.executable,"-m","codex_context.bridge","--config",str(settings.path),"flush"],
                stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
        elif args.command=="flush": print(canonical(flush(settings,args.session)))
        else: print(json.dumps(status(settings,args.session),indent=2))
    except (ValueError,RuntimeError,OSError,KeyError) as error:
        print(type(error).__name__+": "+str(error),file=sys.stderr)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
