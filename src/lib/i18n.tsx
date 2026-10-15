import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { en } from "@/lib/messages/en";
import { zh } from "@/lib/messages/zh";

export type Lang = "en" | "zh";
export type LangPref = "system" | Lang;
export type MessageKey = keyof typeof en;
export type Vars = Record<string, string | number>;

export const LANGS: Record<Lang, Record<MessageKey, string>> = { en, zh };

const STORAGE_KEY = "skillshub.lang";

export function resolveLang(pref: LangPref, systemLangs: readonly string[]): Lang {
  if (pref !== "system") return pref;
  const first = systemLangs[0]?.toLowerCase() ?? "en";
  return first.startsWith("zh") ? "zh" : "en";
}

export function translate(lang: Lang, key: MessageKey, vars?: Vars): string {
  const template = LANGS[lang][key];
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => (name in vars ? String(vars[name]) : whole));
}

const DAY = 86_400_000;

export function relativeTime(iso: string, lang: Lang, now = Date.now()): string {
  const rtf = new Intl.RelativeTimeFormat(lang === "zh" ? "zh-CN" : "en", { numeric: "auto" });
  const diff = Date.parse(iso) - now;
  const abs = Math.abs(diff);
  if (abs < 3_600_000) return rtf.format(Math.round(diff / 60_000), "minute");
  if (abs < DAY) return rtf.format(Math.round(diff / 3_600_000), "hour");
  if (abs < 30 * DAY) return rtf.format(Math.round(diff / DAY), "day");
  if (abs < 365 * DAY) return rtf.format(Math.round(diff / (30 * DAY)), "month");
  return rtf.format(Math.round(diff / (365 * DAY)), "year");
}

function readPref(): LangPref {
  const v = localStorage.getItem(STORAGE_KEY);
  return v === "en" || v === "zh" ? v : "system";
}

interface I18n {
  lang: Lang;
  pref: LangPref;
  setPref: (next: LangPref) => void;
  t: (key: MessageKey, vars?: Vars) => string;
  rel: (iso: string | null, now?: number) => string;
  date: (iso: string | null) => string;
}

const Ctx = createContext<I18n | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [pref, setPrefState] = useState<LangPref>(readPref);
  const lang = resolveLang(pref, navigator.languages);

  useEffect(() => {
    document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
  }, [lang]);

  const setPref = useCallback((next: LangPref) => {
    localStorage.setItem(STORAGE_KEY, next);
    setPrefState(next);
  }, []);

  const value = useMemo<I18n>(
    () => ({
      lang,
      pref,
      setPref,
      t: (key, vars) => translate(lang, key, vars),
      rel: (iso, now) => (iso ? relativeTime(iso, lang, now) : translate(lang, "common.never")),
      date: (iso) => (iso ? new Date(iso).toLocaleDateString("en-CA") : translate(lang, "common.unknown")),
    }),
    [lang, pref, setPref],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useI18n(): I18n {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useI18n must be used inside I18nProvider");
  return ctx;
}
