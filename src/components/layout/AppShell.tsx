// src/components/layout/AppShell.tsx
import type { ReactNode } from "react";
import { TitleBar } from "./TitleBar";
import { LeftSidebar } from "./LeftSidebar";
import { RightSidebar } from "./RightSidebar";
import { ResizeHandle } from "./ResizeHandle";
import { useNavigation } from "@/contexts/NavigationContext";
import { useSettings } from "@/contexts/SettingsContext";

interface AppShellProps {
  children: ReactNode;
  bottom?: ReactNode;
}

export function AppShell({ children, bottom }: AppShellProps) {
  const { activeSection } = useNavigation();
  const {
    leftSidebarWidth,
    rightSidebarWidth,
    setLeftSidebarWidth,
    setRightSidebarWidth,
  } = useSettings();

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        {activeSection === "home" && (
          <>
            <div
              style={{ width: leftSidebarWidth }}
              className="flex-shrink-0 border-r border-border bg-surface overflow-hidden min-h-0"
            >
              <LeftSidebar />
            </div>
            <ResizeHandle
              side="left"
              onResize={(d) =>
                setLeftSidebarWidth((w) => Math.max(160, Math.min(320, w + d)))
              }
              onReset={() => setLeftSidebarWidth(260)}
            />
          </>
        )}
        <main className="flex-1 overflow-auto p-6">{children}</main>
        {activeSection === "home" && (
          <>
            <ResizeHandle
              side="right"
              onResize={(d) =>
                setRightSidebarWidth((w) => Math.max(200, Math.min(400, w - d)))
              }
              onReset={() => setRightSidebarWidth(260)}
            />
            <div
              style={{ width: rightSidebarWidth }}
              className="flex-shrink-0 border-l border-border bg-surface overflow-hidden min-h-0"
            >
              <RightSidebar />
            </div>
          </>
        )}
      </div>
      {bottom}
    </div>
  );
}