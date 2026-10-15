import { describe, expect, it } from "vitest";
import { LANGS, relativeTime, resolveLang, translate, type MessageKey } from "./i18n";

const NOW = Date.parse("2026-10-15T12:00:00Z");
const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe("language choice", () => {
  it("follows the system only when the preference is system", () => {
    expect(resolveLang("system", ["zh-CN", "en-US"])).toBe("zh");
    expect(resolveLang("system", ["zh-Hant-TW"])).toBe("zh");
    expect(resolveLang("system", ["fr-FR"])).toBe("en");
    expect(resolveLang("system", [])).toBe("en");
    expect(resolveLang("en", ["zh-CN"])).toBe("en");
    expect(resolveLang("zh", ["en-US"])).toBe("zh");
  });
});

describe("catalog", () => {
  const keys = Object.keys(LANGS.en) as MessageKey[];

  it("has the same keys in both languages", () => {
    expect(Object.keys(LANGS.zh).sort()).toEqual([...keys].sort());
  });

  it("uses the same placeholders in both languages", () => {
    for (const key of keys) {
      expect(placeholders(LANGS.zh[key]), key).toEqual(placeholders(LANGS.en[key]));
    }
  });

  it("leaves no empty message", () => {
    for (const lang of ["en", "zh"] as const) {
      for (const key of keys) expect(LANGS[lang][key].trim(), `${lang}:${key}`).not.toBe("");
    }
  });

  it("fills placeholders and keeps unknown ones visible", () => {
    expect(translate("en", "common.never")).toBe("never");
    expect(translate("zh", "common.never")).toBe("从未");
  });
});

describe("relative time", () => {
  it("renders in the chosen language", () => {
    const threeMonthsAgo = new Date(NOW - 90 * 86_400_000).toISOString();
    expect(relativeTime(threeMonthsAgo, "en", NOW)).toBe("3 months ago");
    expect(relativeTime(threeMonthsAgo, "zh", NOW)).toBe("3个月前");
    expect(relativeTime(new Date(NOW - 86_400_000).toISOString(), "zh", NOW)).toBe("昨天");
  });
});
