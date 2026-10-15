import { useEffect, useState } from "react";

export type ThemePref = "system" | "light" | "dark";

const KEY = "skillshub.theme";
const query = () => window.matchMedia("(prefers-color-scheme: dark)");

export function resolveTheme(pref: ThemePref, systemDark: boolean): "light" | "dark" {
  return pref === "system" ? (systemDark ? "dark" : "light") : pref;
}

function readPref(): ThemePref {
  const v = localStorage.getItem(KEY);
  return v === "light" || v === "dark" ? v : "system";
}

export function useTheme() {
  const [pref, setPref] = useState<ThemePref>(readPref);

  useEffect(() => {
    const apply = () => {
      document.documentElement.dataset.theme = resolveTheme(pref, query().matches);
    };
    apply();
    const mq = query();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [pref]);

  const choose = (next: ThemePref) => {
    localStorage.setItem(KEY, next);
    setPref(next);
  };
  return [pref, choose] as const;
}
