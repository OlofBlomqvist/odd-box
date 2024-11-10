import { useEffect, useRef, useState } from "react";
import { TabsTrigger } from "../tabs";
import { useRouter } from "@tanstack/react-router";

export const SlidingTabBar = ({
  tabs,
  defaultTabIndex,
}: {
  defaultTabIndex?: number;
  tabs: Array<{ value: string; label: string }>;
}) => {
  const tabsRef = useRef<(HTMLElement | null)[]>([]);
  const router = useRouter();
  const [activeTabIndex, setActiveTabIndex] = useState<number>(
    defaultTabIndex ?? 0
  );
  const [tabUnderlineWidth, setTabUnderlineWidth] = useState(0);
  const [tabUnderlineLeft, setTabUnderlineLeft] = useState(0);
  const [isLoaded, setIsLoaded] = useState(false);

  useEffect(() => {
    if (!tabsRef.current[activeTabIndex]) {
      setActiveTabIndex(0);
      return;
    }

    const setTabPosition = () => {
      const currentTab = tabsRef.current[activeTabIndex] as HTMLElement;
      setTabUnderlineLeft(currentTab?.offsetLeft ?? 0);
      setTabUnderlineWidth(currentTab?.clientWidth ?? 0);
    };

    setTabPosition();
    router.navigate({
      search: {
        type: tabs[activeTabIndex].value,
      },
      replace: true,
    });
  }, [activeTabIndex]);

  useEffect(() => {
    setTimeout(() => setIsLoaded(true), 100);
  }, []);

  return (
    <div className="flew-row relative flex h-12 rounded-md  px-2   backdrop-blur-sm">
      <span
        className={`absolute bottom-0 top-0 -z-10 flex overflow-hidden rounded-md py-2 ${isLoaded ? "transition-all duration-300" : ""}`}
        style={{ left: tabUnderlineLeft, width: tabUnderlineWidth }}
      >
        <span className="h-full w-full rounded-md bg-[var(--card)] border border-[var(--border)]" />
      </span>
      {tabs.map((tab, index) => {
        const isActive = activeTabIndex === index;

        return (
          <TabsTrigger key={index} value={tab.value} asChild>
            <button
              ref={(el) => (tabsRef.current[index] = el)}
              className={`${
                isActive ? `` : `hover:bg-[var(--card)] hover:text-[var(--color)]`
              } my-auto cursor-pointer select-none rounded-md px-4 text-center font-light text-[var(--color)]`}
              onClick={() => setActiveTabIndex(index)}
            >
              {tab.label}
            </button>
          </TabsTrigger>
        );
      })}
    </div>
  );
};
