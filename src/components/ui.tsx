import { useEffect, type ButtonHTMLAttributes, type ReactNode } from "react";
import { X } from "lucide-react";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";

type Variant = "default" | "primary" | "ghost" | "danger";

export function Button({
  variant = "default",
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return (
    <button
      {...props}
      className={cn(
        "inline-flex h-7 items-center gap-1.5 rounded-md px-2.5 text-[12.5px] font-medium transition-colors disabled:pointer-events-none disabled:opacity-40",
        variant === "default" && "border border-line bg-bg hover:bg-hover",
        variant === "primary" && "bg-accent text-bg hover:opacity-85",
        variant === "ghost" && "text-muted hover:bg-hover hover:text-fg",
        variant === "danger" && "border border-line text-danger hover:bg-hover",
        className,
      )}
    />
  );
}

export function Chip({ children, className, title }: { children: ReactNode; className?: string; title?: string }) {
  return (
    <span
      title={title}
      className={cn(
        "inline-flex h-[18px] items-center rounded px-1.5 text-[11px] leading-none text-muted ring-1 ring-inset ring-line",
        className,
      )}
    >
      {children}
    </span>
  );
}

export function Dialog({
  title,
  onClose,
  children,
  width = 480,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  width?: number;
}) {
  const { t } = useI18n();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/30 pt-[14vh]" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-label={title}
        style={{ width }}
        className="max-h-[72vh] overflow-auto rounded-xl border border-line bg-bg shadow-2xl"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-4 pt-3.5 pb-2">
          <h2 className="text-[14px] font-semibold">{title}</h2>
          <button onClick={onClose} aria-label={t("common.close")} className="rounded p-1 text-muted hover:bg-hover hover:text-fg">
            <X size={14} />
          </button>
        </div>
        <div className="px-4 pb-4">{children}</div>
      </div>
    </div>
  );
}
