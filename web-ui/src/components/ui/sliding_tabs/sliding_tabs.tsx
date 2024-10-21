import { useEffect, useRef, useState } from "react";
import { TabsTrigger } from "../tabs";
import { useRouter } from "@tanstack/react-router";

export const SlidingTabBar = ({ tabs }: { tabs: Array<{value:string,label:string}> }) => {
  const tabsRef = useRef<(HTMLElement | null)[]>([]);
  const router = useRouter();
  const searchParams = new URLSearchParams(window.location.search);
  const type = searchParams.get("type");
  const [activeTabIndex, setActiveTabIndex] = useState<number>(type === "sites" ? 1 : 0);
  const [tabUnderlineWidth, setTabUnderlineWidth] = useState(0);
  const [tabUnderlineLeft, setTabUnderlineLeft] = useState(0);
  const [isLoaded, setIsLoaded] = useState(false);

  useEffect(() => {
    const setTabPosition = () => {
      const currentTab = tabsRef.current[activeTabIndex] as HTMLElement;
      setTabUnderlineLeft(currentTab?.offsetLeft ?? 0);
      setTabUnderlineWidth(currentTab?.clientWidth ?? 0);
    };

    setTabPosition();
    router.navigate({
        search: {
            type: tabs[activeTabIndex].value
        },
        replace: true
    })
  }, [activeTabIndex]);

  useEffect(() => {setTimeout(() => setIsLoaded(true), 100)}, [])

  return (
    <div className="flew-row relative flex h-12 rounded-md  px-2   backdrop-blur-sm">
      <span
        className={`absolute bottom-0 top-0 -z-10 flex overflow-hidden rounded-md py-2 ${isLoaded ? "transition-all duration-300" : ''}`}
        style={{ left: tabUnderlineLeft, width: tabUnderlineWidth }}
      >
        <span className="h-full w-full rounded-md bg-gray-200/30" />
      </span>
      {tabs.map((tab, index) => {
        const isActive = activeTabIndex === index;

        return (
            <TabsTrigger key={index} value={tab.value} asChild>

          <button
            
            ref={(el) => (tabsRef.current[index] = el)}
            className={`${
              isActive ? `` : `hover:text-neutral-300 hover:bg-white/5`
            } my-auto cursor-pointer select-none rounded-md px-4 text-center font-light text-white`}
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
