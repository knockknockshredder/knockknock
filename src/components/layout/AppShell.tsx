// src/components/layout/AppShell.tsx
import type { ReactNode } from "react";
import { TitleBar } from "./TitleBar";
import { Sidebar } from "./Sidebar";

interface AppShellProps {
  children: ReactNode;
  bottom?: ReactNode;
}

export function AppShell({ children, bottom }: AppShellProps) {
  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <main className="flex flex-1 flex-col overflow-hidden">
          <div className="flex-1 overflow-auto p-6">{children}</div>
          {bottom}
        </main>
      </div>
    </div>
  );
}
