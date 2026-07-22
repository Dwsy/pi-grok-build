"use client";

import { useI18n } from "@/i18n/provider";
import Link from "next/link";
import { usePathname } from "next/navigation";

interface DocLink {
  href: string;
  label: string;
}

interface DocGroup {
  title: string;
  links: DocLink[];
}

export default function DocsSidebar() {
  const { t } = useI18n();
  const pathname = usePathname();
  const s = t.docs.sidebar;

  const groups: DocGroup[] = [
    {
      title: s.gettingStarted,
      links: [
        { href: "/docs", label: s.quickStart },
        { href: "/docs/installation", label: s.installation },
        { href: "/docs/configuration", label: s.configuration },
      ],
    },
    {
      title: s.coreConcepts,
      links: [
        { href: "/docs/architecture", label: s.architecture },
        { href: "/docs/features", label: s.features },
        { href: "/docs/commands", label: s.commands },
      ],
    },
    {
      title: s.guides,
      links: [
        { href: "/docs/migration", label: s.migration },
      ],
    },
  ];

  return (
    <aside className="hidden lg:block w-64 shrink-0">
      <nav className="sticky top-24 space-y-8">
        {groups.map((group) => (
          <div key={group.title}>
            <h3 className="px-3 mb-2 text-xs font-medium text-text-tertiary">
              {group.title}
            </h3>
            <ul className="space-y-0.5">
              {group.links.map((link) => {
                const active = pathname === link.href;
                return (
                  <li key={link.href}>
                    <Link
                      href={link.href}
                      className={`block px-3 py-1.5 rounded-md text-sm transition-colors ${
                        active
                          ? "bg-accent/10 text-accent-bright font-medium"
                          : "text-text-secondary hover:text-text-primary hover:bg-surface-hover"
                      }`}
                    >
                      {link.label}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </nav>
    </aside>
  );
}
