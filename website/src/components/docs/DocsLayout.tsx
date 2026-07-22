"use client";

import { useI18n } from "@/i18n/provider";
import DocsSidebar from "@/components/docs/DocsSidebar";
import Link from "next/link";

export default function DocsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const { t } = useI18n();

  return (
    <div className="min-h-screen bg-void">
      {/* Docs top bar */}
      <header className="fixed top-0 left-0 right-0 z-50 bg-void/95 border-b border-border">
        <div className="mx-auto max-w-7xl px-6 h-16 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <Link href="/" className="flex items-center gap-2 group">
              <span className="text-xl font-bold font-mono text-accent-bright">π</span>
              <span className="text-sm font-semibold text-text-primary">grok-pi</span>
            </Link>
            <span className="text-text-tertiary">/</span>
            <span className="text-sm text-text-secondary">{t.docs.title}</span>
          </div>
          <div className="flex items-center gap-4">
            <Link
              href="/"
              className="text-sm text-text-secondary hover:text-text-primary transition-colors"
            >
              ← {t.nav.features}
            </Link>
            <a
              href="https://github.com/Dwsy/grok-pi"
              target="_blank"
              rel="noopener noreferrer"
              className="text-sm text-text-secondary hover:text-text-primary transition-colors"
            >
              {t.nav.github}
            </a>
          </div>
        </div>
      </header>

      {/* Content */}
      <div className="pt-16">
        <div className="mx-auto max-w-7xl px-6 py-12 flex gap-12">
          <DocsSidebar />
          <main className="flex-1 min-w-0 max-w-3xl">
            {children}
          </main>
        </div>
      </div>
    </div>
  );
}
