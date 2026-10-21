import { useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowUpRight, Download, FolderOpen, Link2, Trash2, X } from "lucide-react";
import { api } from "@/lib/api";
import { invalidateCatalog } from "@/lib/reload";
import { agentLabel, homeRelative } from "@/lib/format";
import { useI18n } from "@/lib/i18n";
import type { AgentInfo, Skill } from "@/lib/types";
import { Button, Chip, Dialog } from "./ui";
import { FullHeat } from "./Heatmap";

function Section({ title, right, children }: { title: string; right?: ReactNode; children: ReactNode }) {
  return (
    <section className="border-t border-line px-5 py-4">
      <div className="mb-2.5 flex items-center justify-between">
        <h3 className="text-[11px] font-medium tracking-wide text-faint uppercase">{title}</h3>
        {right}
      </div>
      {children}
    </section>
  );
}

function Row({ k, children }: { k: string; children: ReactNode }) {
  return (
    <div className="flex gap-3 py-0.5 text-[12.5px]">
      <div className="w-[76px] shrink-0 text-muted">{k}</div>
      <div className="selectable min-w-0 flex-1 break-words">{children}</div>
    </div>
  );
}

function stripFrontmatter(md: string): string {
  return md.replace(/^\uFEFF?---\r?\n[\s\S]*?\r?\n---\r?\n?/, "").trim();
}

export function DetailPanel({
  skill,
  agents,
  onClose,
  onInstallElsewhere,
}: {
  skill: Skill;
  agents: AgentInfo[];
  onClose: () => void;
  onInstallElsewhere: (s: Skill) => void;
}) {
  const { t, rel, date } = useI18n();
  const qc = useQueryClient();
  const label = (id: string) => agentLabel(id, agents, t);
  const uses = (n: number) => t(n === 1 ? "common.uses.one" : "common.uses.other", { n });
  const detail = useQuery({ queryKey: ["skill", skill.id], queryFn: () => api.detail(skill.id) });
  const [pending, setPending] = useState<{ paths: string[]; scope?: string[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [mdOpen, setMdOpen] = useState(false);
  const [descOpen, setDescOpen] = useState(false);
  const [tagDraft, setTagDraft] = useState("");

  const refresh = () => invalidateCatalog(qc);

  const tagMut = useMutation({
    mutationFn: (tags: string[]) => api.setTags(skill.id, tags),
    onSuccess: refresh,
  });

  const askRemove = async (scope?: string[]) => {
    setError(null);
    try {
      setPending({ paths: await api.planRemoval(skill.id, scope), scope });
    } catch (e) {
      setError(String(e));
    }
  };

  const removeMut = useMutation({
    mutationFn: () => api.remove(skill.id, pending?.scope),
    onSuccess: async () => {
      setPending(null);
      await refresh();
    },
    onError: (e) => setError(String(e)),
  });

  const pluginOnly = skill.installs.every((i) => i.plugin);
  const body = detail.data ? stripFrontmatter(detail.data.skillMd) : "";
  const usedBy = Object.entries(skill.usage.byAgent).sort((a, b) => b[1] - a[1]);
  const addTag = () => {
    const tag = tagDraft.trim();
    if (tag && !skill.tags.includes(tag)) tagMut.mutate([...skill.tags, tag]);
    setTagDraft("");
  };

  return (
    <aside className="flex h-full w-[420px] shrink-0 flex-col border-l border-line bg-panel">
      <div data-tauri-drag-region className="flex h-[52px] shrink-0 items-end justify-end px-3 pb-2">
        <button onClick={onClose} aria-label={t("detail.close")} className="rounded p-1 text-muted hover:bg-hover hover:text-fg">
          <X size={15} />
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="px-5 pb-4">
          <h2 className="selectable text-[18px] leading-tight font-semibold">{skill.name}</h2>
          <p className={`selectable mt-1.5 text-[13px] leading-snug text-muted ${descOpen ? "" : "line-clamp-4"}`}>
            {skill.description || t("detail.noDescription")}
          </p>
          {skill.description.length > 220 && (
            <button onClick={() => setDescOpen((v) => !v)} className="mt-1 text-[12px] text-muted hover:text-fg">
              {descOpen ? t("common.showLess") : t("common.showMore")}
            </button>
          )}
          <div className="mt-3 flex flex-wrap gap-1.5">
            <Button onClick={() => api.reveal(skill.canonicalDir)}>
              <FolderOpen size={13} /> {t("detail.reveal")}
            </Button>
            <Button onClick={() => onInstallElsewhere(skill)} disabled={!skill.source || skill.source.kind === "plugin"}>
              <Download size={13} /> {t("detail.installTo")}
            </Button>
            <Button
              variant="danger"
              disabled={pluginOnly}
              title={pluginOnly ? t("detail.deleteManaged") : t("remove.confirm")}
              onClick={() => askRemove()}
            >
              <Trash2 size={13} /> {t("common.delete")}
            </Button>
          </div>
          {error && <div className="selectable mt-2 text-[12px] text-danger">{error}</div>}
        </div>

        <Section
          title={t("detail.activity")}
          right={<span className="text-[11px] text-muted">{uses(skill.usage.total)}</span>}
        >
          <FullHeat daily={skill.usage.daily} />
          <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-[12px] text-muted">
            <span>{t("detail.lastUsed", { when: rel(skill.usage.lastUsed) })}</span>
            {usedBy.map(([a, n]) => (
              <span key={a}>
                {label(a)} <span className="text-fg tabular-nums">{n}</span>
              </span>
            ))}
          </div>
        </Section>

        <Section title={t("detail.details")}>
          <Row k={t("common.source")}>
            {skill.source ? (
              skill.source.url ? (
                <button
                  className="inline-flex items-center gap-1 hover:underline"
                  onClick={() => openUrl(skill.source!.url)}
                >
                  {skill.source.repo} <ArrowUpRight size={12} className="text-muted" />
                </button>
              ) : (
                skill.source.repo
              )
            ) : (
              <span className="text-muted">{t("detail.unknownLocal")}</span>
            )}
          </Row>
          {skill.source && <Row k={t("detail.author")}>{skill.source.author}</Row>}
          <Row k={t("common.installed")}>{date(skill.installedAt)}</Row>
          {skill.updatedAt && skill.updatedAt !== skill.installedAt && <Row k={t("detail.updated")}>{date(skill.updatedAt)}</Row>}
        </Section>

        <Section title={t("detail.installedIn")}>
          <ul className="space-y-1">
            {skill.installs.map((i) => (
              <li key={i.path} className="group flex items-center gap-2 text-[12px]">
                <Chip className="shrink-0">{label(i.agent)}</Chip>
                {i.kind === "symlink" && <Link2 size={12} className="shrink-0 text-faint" aria-label={t("detail.symlink")} />}
                <button
                  onClick={() => api.reveal(i.path)}
                  title={i.path}
                  className="selectable min-w-0 flex-1 truncate text-left text-muted hover:text-fg"
                >
                  {homeRelative(i.path)}
                </button>
                {!i.plugin && skill.installs.length > 1 && (
                  <button
                    onClick={() => askRemove([i.path])}
                    aria-label={t("detail.removeFrom", { agent: label(i.agent) })}
                    className="shrink-0 rounded p-1 text-faint opacity-0 group-hover:opacity-100 hover:bg-hover hover:text-danger"
                  >
                    <Trash2 size={12} />
                  </button>
                )}
              </li>
            ))}
          </ul>
        </Section>

        <Section title={t("common.tags")}>
          <div className="flex flex-wrap items-center gap-1.5">
            {skill.autoTags.map((tag) => (
              <Chip key={`a-${tag}`} title={t("detail.autoTag")}>
                {tag}
              </Chip>
            ))}
            {skill.tags.map((tag) => (
              <Chip key={tag} className="gap-1 text-fg">
                {tag}
                <button
                  aria-label={t("detail.removeTag", { tag })}
                  onClick={() => tagMut.mutate(skill.tags.filter((x) => x !== tag))}
                  className="text-faint hover:text-danger"
                >
                  <X size={10} />
                </button>
              </Chip>
            ))}
            <input
              value={tagDraft}
              onChange={(e) => setTagDraft(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addTag()}
              onBlur={addTag}
              placeholder={t("detail.addTag")}
              className="h-[22px] w-[84px] rounded bg-transparent px-1 text-[12px] outline-none placeholder:text-faint focus:bg-hover"
            />
          </div>
        </Section>

        <Section
          title="SKILL.md"
          right={
            body && (
              <button onClick={() => setMdOpen((v) => !v)} className="text-[11px] text-muted hover:text-fg">
                {mdOpen ? t("common.collapse") : t("common.expand")}
              </button>
            )
          }
        >
          {detail.isLoading ? (
            <div className="text-[12px] text-faint">{t("common.loading")}</div>
          ) : (
            <pre
              className={`font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap text-muted ${mdOpen ? "" : "max-h-40 overflow-hidden mask-b-from-60%"}`}
            >
              {body || t("detail.emptyFile")}
            </pre>
          )}
          {detail.data && detail.data.files.length > 1 && (
            <div className="mt-3 text-[11.5px] text-faint">
              {t("detail.files", { n: detail.data.files.length, list: detail.data.files.slice(0, 6).join(", ") })}
              {detail.data.files.length > 6 && "…"}
            </div>
          )}
        </Section>
      </div>

      {pending && (
        <Dialog title={t("remove.title")} onClose={() => setPending(null)}>
          <p className="text-[13px] text-muted">
            {skill.source && pending.paths.length === skill.installs.length ? t("remove.bodyWithLock") : t("remove.body")}
          </p>
          <ul className="selectable mt-3 space-y-1 rounded-md bg-hover p-2.5 font-mono text-[11.5px]">
            {pending.paths.map((p) => (
              <li key={p} className="break-all">
                {homeRelative(p)}
              </li>
            ))}
          </ul>
          <div className="mt-4 flex justify-end gap-2">
            <Button onClick={() => setPending(null)}>{t("common.cancel")}</Button>
            <Button variant="primary" onClick={() => removeMut.mutate()} disabled={removeMut.isPending}>
              {removeMut.isPending ? t("remove.moving") : t("remove.confirm")}
            </Button>
          </div>
        </Dialog>
      )}
    </aside>
  );
}
