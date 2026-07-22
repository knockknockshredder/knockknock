// src/components/layout/AppShell.tsx
import { useState, useEffect } from "react";
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

function pctToPx(percent: number): number {
  return (percent / 100) * window.innerWidth;
}

export function AppShell({ children, bottom }: AppShellProps) {
  const { activeSection } = useNavigation();
  const {
    leftSidebarWidth,
    rightSidebarWidth,
    setLeftSidebarWidth,
    setRightSidebarWidth,
  } = useSettings();

  // Convert percentage widths to pixel values, recalculate on resize
  const [pixelWidths, setPixelWidths] = useState(() => ({
    left: pctToPx(leftSidebarWidth),
    right: pctToPx(rightSidebarWidth),
  }));

  useEffect(() => {
    function handleResize() {
      setPixelWidths({
        left: pctToPx(leftSidebarWidth),
        right: pctToPx(rightSidebarWidth),
      });
    }
    handleResize();
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, [leftSidebarWidth, rightSidebarWidth]);

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        {activeSection === "home" && (
          <>
            <div
              style={{ width: pixelWidths.left }}
              className="flex-shrink-0 border-r border-border bg-surface overflow-hidden min-h-0"
            >
              <LeftSidebar />
            </div>
            <ResizeHandle
              side="left"
              onResize={(d) =>
                setLeftSidebarWidth((w) =>
                  Math.max(20, Math.min(33.33, w + (d / window.innerWidth) * 100)),
                )
              }
              onReset={() => setLeftSidebarWidth(33.33)}
            />
          </>
        )}
        <main className="flex-1 overflow-auto p-6 flex justify-center">
          {children}
        </main>
        {activeSection === "home" && (
          <>
            <ResizeHandle
              side="right"
              onResize={(d) =>
                setRightSidebarWidth((w) =>
                  Math.max(20, Math.min(33.33, w - (d / window.innerWidth) * 100)),
                )
              }
              onReset={() => setRightSidebarWidth(33.33)}
            />
            <div
              style={{ width: pixelWidths.right }}
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