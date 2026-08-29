import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type { CuratorRun, SharedObject, Task } from "./types";

const ids = {
  task: "11111111-1111-4111-8111-111111111111",
  chat: "22222222-2222-4222-8222-222222222222",
  user: "33333333-3333-4333-8333-333333333333",
  entity: "44444444-4444-4444-8444-444444444444",
  memory: "55555555-5555-4555-8555-555555555555",
  connection: "66666666-6666-4666-8666-666666666666",
  run: "77777777-7777-4777-8777-777777777777",
  message: "88888888-8888-4888-8888-888888888888",
  eval: "99999999-9999-4999-8999-999999999999",
};

const now = "2026-08-29T00:00:00Z";
const objects: SharedObject[] = ([
  [ids.task, "task", "Canonical task"],
  [ids.chat, "chat", "Canonical chat"],
  [ids.user, "user", "Canonical user"],
  [ids.entity, "entity", "Canonical entity"],
  [ids.memory, "memory", "Canonical memory"],
] as const).map(([id, kind, title]) => ({
  id, kind, title, description: `${title} description`, protected: false, lifecycle: "active", revision: 1,
  created_by_type: "human", created_by_id: "local-human", updated_by_type: "human", updated_by_id: "local-human",
  provenance: { source_type: "human" }, created_at: now, updated_at: now, archived_at: null,
}));

const task: Task = {
  object_id: ids.task, title: "Canonical task", description: "Canonical task description", lifecycle: "active", revision: 1,
  provenance: { source_type: "human" }, protected: false, status: "todo", priority: "medium", owner_object_id: ids.user,
  agent_eligible: true, due_at: null, created_at: now, updated_at: now,
};

const run: CuratorRun = {
  id: ids.run, chat_object_id: ids.chat, first_message_id: ids.message, last_message_id: ids.message,
  trigger: "explicit_finish", status: "completed", message_count: 1, idempotency_key: "run", attempts: 1,
  worker_id: null, model: "test", prompt_version: "v1", proposed_plan: null, committed_plan: {}, result: {},
  created_at: now, started_at: now, completed_at: now, reversed_at: null, error_message: null,
};

const evalSummary = {
  id: ids.eval, kind: "slack_interaction", status: "completed", actor_type: "system", actor_id: "chat-ingestor",
  chat_object_id: ids.chat, curator_run_id: ids.run, summary: "Slack interaction for Canonical chat", error_summary: null,
  verdict: "mixed", notes: "Useful result with one correction.", annotated_by: "local-human", annotation_revision: 1,
  affected_object_count: 2, total_tokens: 180, estimated_micro_usd: 1200, chatgpt_credit_microunits: null,
  usage_sources: [
    { component: "centaur_agent", provider: "openai", model_id: "gpt-5.6-sol", display_tier: "GPT-5.6 Sol", execution_type: "codex_harness", auth_mode: "chatgpt_subscription", billing_mode: "subscription_allowance", usage_status: "reported" },
    { component: "context_curator", provider: "openai", model_id: "gpt-4.1-mini", display_tier: "GPT-4.1 mini", execution_type: "direct_api", auth_mode: "api_key", billing_mode: "metered_api", usage_status: "reported" },
  ],
  created_at: now, updated_at: now, completed_at: now,
} as const;

const visuals = objects.map((object) => ({
  object_id: object.id,
  source_provider: object.id === ids.chat || object.id === ids.memory ? "slack" : null,
  users: object.id === ids.chat || object.id === ids.memory || object.id === ids.user ? [{
    object_id: object.id,
    user_object_id: ids.user,
    title: "Canonical user",
    user_kind: "human",
    role: object.id === ids.user ? "identity" : object.id === ids.chat ? "participant" : "source author",
    avatar_url: null,
  }] : [],
}));

function json(data: unknown) {
  return Promise.resolve(new Response(JSON.stringify({ data }), { status: 200, headers: { "Content-Type": "application/json" } }));
}

function installApiMock() {
  vi.stubGlobal("fetch", vi.fn((input: string | URL | Request) => {
    const path = typeof input === "string" ? input : input instanceof URL ? input.pathname + input.search : new URL(input.url).pathname;
    if (path.startsWith("/api/v1/objects?")) return json(objects);
    if (path === "/api/v1/object-visuals") return json(visuals);
    if (path === "/api/v1/tasks") return json([task]);
    if (path === `/api/v1/tasks/${ids.task}`) return json(task);
    if (path === "/api/v1/curator-runs") return json([run]);
    if (path === `/api/v1/curator-runs/${ids.run}`) return json({
      run,
      messages: [{ id: ids.message, chat_object_id: ids.chat, provider_message_id: "1", sender_user_object_id: ids.user, sender_title: "Canonical user", sender_kind: "human", content: "Hello", source_created_at: now, ingested_sequence: 1, ingested_at: now }],
      changes: [
        { id: "change-object", sequence: 1, entity_type: "object", entity_id: ids.memory, action: "created", before_state: null, after_state: { title: "Canonical memory" }, after_revision: 1, created_at: now, undone_at: null },
        { id: "change-connection", sequence: 2, entity_type: "connection", entity_id: ids.connection, action: "created", before_state: null, after_state: {}, after_revision: 1, created_at: now, undone_at: null },
      ],
    });
    if (path.startsWith("/api/v1/evals?")) return json([evalSummary]);
    if (path === `/api/v1/evals/${ids.eval}`) return json({
      eval: evalSummary,
      trace: [
        { id: "trace-1", eval_id: ids.eval, sequence: 1, entry_type: "message_ingested", component: null, provider: null, model_id: null, display_tier: null, execution_type: null, auth_mode: null, upstream_service: null, billing_mode: null, reasoning_effort: null, service_tier: null, source_thread_id: null, source_execution_id: null, source_turn_id: null, usage_status: "not_applicable", usage_missing_reason: null, input_tokens: null, output_tokens: null, cache_creation_tokens: null, cache_read_tokens: null, reasoning_tokens: null, total_tokens: null, estimated_micro_usd: null, chatgpt_credit_microunits: null, api_equivalent_micro_usd: null, rate_card_version: null, pricing_snapshot: null, facts: { message_id: ids.message }, created_at: now },
        { id: "trace-2", eval_id: ids.eval, sequence: 2, entry_type: "model_attempt", component: "centaur_agent", provider: "openai", model_id: "gpt-5.6-sol", display_tier: "GPT-5.6 Sol", execution_type: "codex_harness", auth_mode: "chatgpt_subscription", upstream_service: "chatgpt.com", billing_mode: "subscription_allowance", reasoning_effort: "high", service_tier: null, source_thread_id: "thread", source_execution_id: "execution", source_turn_id: "turn", usage_status: "reported", usage_missing_reason: null, input_tokens: 100, output_tokens: 50, cache_creation_tokens: 0, cache_read_tokens: 20, reasoning_tokens: 10, total_tokens: 150, estimated_micro_usd: null, chatgpt_credit_microunits: null, api_equivalent_micro_usd: null, rate_card_version: null, pricing_snapshot: null, facts: {}, created_at: now },
        { id: "trace-3", eval_id: ids.eval, sequence: 3, entry_type: "model_attempt", component: "context_curator", provider: "openai", model_id: "gpt-4.1-mini", display_tier: "GPT-4.1 mini", execution_type: "direct_api", auth_mode: "api_key", upstream_service: "api.openai.com", billing_mode: "metered_api", reasoning_effort: null, service_tier: null, source_thread_id: "thread", source_execution_id: "curator-execution", source_turn_id: "curator-turn", usage_status: "reported", usage_missing_reason: null, input_tokens: 20, output_tokens: 10, cache_creation_tokens: 0, cache_read_tokens: 0, reasoning_tokens: 0, total_tokens: 30, estimated_micro_usd: 1200, chatgpt_credit_microunits: null, api_equivalent_micro_usd: null, rate_card_version: "fixture-v1", pricing_snapshot: {}, facts: {}, created_at: now },
      ],
      objects: [{ object_id: ids.memory, role: "created", kind: "memory", title: "Canonical memory", lifecycle: "active" }],
    });
    if (path === `/api/v1/evals/${ids.eval}/annotation`) return json({ ...evalSummary, verdict: "pass", annotation_revision: 2 });
    if (path === `/api/v1/connections/${ids.connection}`) return json({ id: ids.connection, source_object_id: ids.chat, kind: "about", target_object_id: ids.memory, description: "The chat is about this memory.", protected: false, revision: 1, created_by_type: "human", created_by_id: "local-human", updated_by_type: "human", updated_by_id: "local-human", provenance: {}, created_at: now, updated_at: now, archived_at: null });
    if (path === `/api/v1/users/${ids.user}`) return json({ object_id: ids.user, title: "Canonical user", description: "Canonical user description", lifecycle: "active", revision: 1, provenance: {}, user_kind: "human", created_at: now, updated_at: now });
    if (path === `/api/v1/users/${ids.user}/identities`) return json([]);
    if (path === `/api/v1/chats/${ids.chat}/messages`) return json([]);
    const objectMatch = path.match(/^\/api\/v1\/objects\/([^/]+)$/);
    if (objectMatch) {
      const item = objects.find((candidate) => candidate.id === objectMatch[1]);
      return item ? json(item) : Promise.resolve(new Response(JSON.stringify({ error: { code: "not_found", message: "Record not found" } }), { status: 404, headers: { "Content-Type": "application/json" } }));
    }
    if (path.endsWith("/connections") || path.endsWith("/events")) return json([]);
    throw new Error(`Unexpected API request: ${path}`);
  }));
}

describe("canonical Object identity across the application", () => {
  beforeEach(() => installApiMock());
  afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

  it("shows a compact copyable canonical Object ID in all six primary lists and Curator rows", async () => {
    window.history.replaceState({}, "", "/objects");
    render(<App />);
    const firstPill = await screen.findByRole("button", { name: `Copy Object ID ${ids.task}` });
    expect(firstPill).toHaveTextContent(`ID: ${ids.task.slice(0, 5)}`);
    const firstTitle = firstPill.parentElement?.nextElementSibling;
    expect(firstTitle).toHaveTextContent("Canonical task");
    expect(firstTitle).toHaveTextContent("Task");
    expect(screen.queryByText("···")).not.toBeInTheDocument();
    for (const [section, id] of [["Tasks", ids.task], ["Chats", ids.chat], ["Users", ids.user], ["Entities", ids.entity], ["Memories", ids.memory], ["Curator Runs", ids.chat]] as const) {
      await userEvent.click(screen.getByRole("button", { name: section }));
      expect(await screen.findByRole("button", { name: `Copy Object ID ${id}` })).toHaveTextContent(`ID: ${id.slice(0, 5)}`);
    }
  });

  it("opens canonical Object IDs at durable URLs and supports back navigation", async () => {
    window.history.replaceState({}, "", "/tasks");
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "Open Canonical task" }));
    expect(window.location.pathname).toBe(`/tasks/${ids.task}`);
    await userEvent.click(await screen.findByRole("link", { name: `Open Object ID ${ids.task}` }));
    expect(window.location.pathname).toBe(`/objects/${ids.task}`);
    expect(await screen.findByRole("heading", { name: "Properties" })).toBeVisible();
    window.history.back();
    await waitFor(() => expect(window.location.pathname).toBe(`/tasks/${ids.task}`));
    expect(await screen.findByRole("heading", { name: "Properties" })).toBeVisible();
  });

  it("shows the full, copyable canonical UUID on every primary detail type", async () => {
    for (const [path, id] of [[`/tasks/${ids.task}`, ids.task], [`/chats/${ids.chat}`, ids.chat], [`/users/${ids.user}`, ids.user], [`/entities/${ids.entity}`, ids.entity], [`/memories/${ids.memory}`, ids.memory]] as const) {
      window.history.replaceState({}, "", path);
      const view = render(<App />);
      expect((await screen.findAllByRole("button", { name: `Copy Object ID ${id}` })).length).toBeGreaterThan(0);
      view.unmount();
    }
  });

  it("keeps detail identity in the header and provenance collapsed at the bottom", async () => {
    window.history.replaceState({}, "", `/memories/${ids.memory}`);
    render(<App />);
    const title = await screen.findByRole("textbox", { name: "Object title" });
    expect(title.parentElement).toHaveClass("detail-heading");
    expect(title.parentElement).toHaveTextContent(`ID: ${ids.memory.slice(0, 5)}`);
    const properties = screen.getByLabelText("Object properties");
    expect(properties).toHaveTextContent("Memory");
    expect(properties).not.toHaveTextContent("Revision");
    expect(properties).not.toHaveTextContent("Protected");
    const provenance = screen.getByText("Provenance").closest("details");
    expect(provenance).not.toHaveAttribute("open");
    expect(provenance?.previousElementSibling).toHaveTextContent("Activity");
  });

  it("routes Curator object and Connection changes to the correct record types", async () => {
    window.history.replaceState({}, "", `/curator-runs/${ids.run}`);
    render(<App />);
    expect(await screen.findByRole("link", { name: `Open Object ID ${ids.memory}` })).toHaveAttribute("href", `/objects/${ids.memory}`);
    expect(screen.getByRole("link", { name: `Open Connection ID ${ids.connection}` })).toHaveAttribute("href", `/connections/${ids.connection}`);
  });

  it("shows both canonical endpoint Objects on a supporting Connection detail route", async () => {
    window.history.replaceState({}, "", `/connections/${ids.connection}`);
    render(<App />);
    expect(await screen.findByRole("link", { name: `Open Object ID ${ids.chat}` })).toBeVisible();
    expect(screen.getByRole("link", { name: `Open Object ID ${ids.memory}` })).toBeVisible();
    expect(screen.getByText("Connection ID")).toBeVisible();
    await screen.findByText("Canonical chat");
    expect(screen.getByLabelText("Source Object")).toHaveTextContent("Canonical chat");
    expect(screen.getByLabelText("Target Object")).toHaveTextContent("Canonical memory");
    expect(screen.getAllByLabelText("Source: Slack")).toHaveLength(2);
  });

  it("shows type, Slack source, and evidence-backed User visuals", async () => {
    window.history.replaceState({}, "", "/memories");
    render(<App />);
    expect(await screen.findByText("Memory")).toBeVisible();
    expect(screen.getByLabelText("Source: Slack")).toBeVisible();
    expect(screen.getByRole("img", { name: "Canonical user, Human, source author" })).toBeVisible();
  });

  it("shows canonical description previews in all six primary lists", async () => {
    window.history.replaceState({}, "", "/objects");
    render(<App />);
    await screen.findByText("Canonical task description");
    for (const [section, description] of [
      ["Tasks", "Canonical task description"],
      ["Chats", "Canonical chat description"],
      ["Users", "Canonical user description"],
      ["Entities", "Canonical entity description"],
      ["Memories", "Canonical memory description"],
    ] as const) {
      await userEvent.click(screen.getByRole("button", { name: section }));
      expect(await screen.findByText(description)).toHaveAccessibleName(
        new RegExp(`Description preview: ${description}`),
      );
    }
  });

  it("gives human description forms concrete type-specific guidance", async () => {
    window.history.replaceState({}, "", "/entities");
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "New entity" }));
    expect(screen.getByLabelText("Entity description")).toHaveAttribute(
      "aria-describedby",
      "new-object-description-help",
    );
    expect(screen.getByText(/Describe this specific entity directly/)).toBeVisible();
  });

  it("renders an explicit missing-target state for an unknown deep link", async () => {
    window.history.replaceState({}, "", "/objects/99999999-9999-4999-8999-999999999999");
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Could not load" })).toBeVisible();
    expect(screen.getByText("Record not found")).toBeVisible();
  });

  it("uses one simple search and compact single-row Eval records", async () => {
    window.history.replaceState({}, "", "/evals");
    render(<App />);
    expect(await screen.findByText("Slack interaction for Canonical chat")).toBeVisible();
    expect(screen.getByText(/Included subscription usage; per-trace USD unavailable/)).toBeVisible();
    expect(screen.getByText(/Metered API estimate \$0\.001200 USD/)).toBeVisible();
    expect(screen.queryByText("$0")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Eval filters")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: `Open Eval ID ${ids.eval}` })).toHaveTextContent(`ID: ${ids.eval.slice(0, 5)}`);
    const search = screen.getByRole("textbox", { name: "Search evals" });
    await userEvent.type(search, "GPT-4.1 mini");
    expect(screen.getByText("Slack interaction for Canonical chat")).toBeVisible();
    await userEvent.clear(search);
    await userEvent.type(search, "no matching eval");
    expect(screen.getByText("No evals match this search.")).toBeVisible();
  });

  it("renders ordered trace, Object navigation, and human review controls", async () => {
    window.history.replaceState({}, "", `/evals/${ids.eval}`);
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Slack interaction for Canonical chat" })).toBeVisible();
    expect(screen.getByText("Ordered trace")).toBeVisible();
    expect(screen.getAllByText("model attempt")).toHaveLength(2);
    expect(screen.getByText(/included subscription usage; per-trace USD unavailable/)).toBeVisible();
    expect(screen.getByText(/estimated \$0\.001200 USD \(fixture-v1\)/)).toBeVisible();
    expect(screen.getByRole("link", { name: `Open Object ID ${ids.memory}` })).toHaveAttribute("href", `/objects/${ids.memory}`);
    expect(screen.queryByText("Canonical memory")).not.toBeInTheDocument();
    expect(screen.getAllByText("model attempt")[0].closest("article")?.children).toHaveLength(4);
    expect(screen.getByRole("combobox", { name: "Verdict" })).toHaveValue("mixed");
    expect(screen.getByRole("textbox", { name: "Review notes" })).toHaveValue("Useful result with one correction.");
  });
});
