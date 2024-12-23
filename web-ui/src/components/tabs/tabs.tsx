import { useRouter } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { TTab } from "./types";
import { cn } from "@/lib/cn";

const Tabs = ({ sections }: { sections?: TTab[] }) => {
  const router = useRouter();
  
  const searchParams = new URLSearchParams(window.location.search);
  const tab = searchParams.get("tab");

  const [tabIndex, setTabIndex] = useState(Number(tab) ?? 0);

  useEffect(() => {
    if (router.state.location.search.tab !== undefined) {
      setTabIndex(router.state.location.search.tab)
    } else {
      setTabIndex(0)
    }
  },[router.state.location.search.tab])

  return (
    <>
      <div style={{ display: "flex", gap: "20px" }} className="pl-[20px] md:pl-0">
        {sections?.map((section, index) => (
          <TabItem
            key={index}
            active={tabIndex === index}
            onClick={() => {
              router.navigate({ search: (x) => ({ ...x,tab: index }) });
              setTabIndex(index);
            }}
            title={section.name}
          />
        ))}
      </div>
      <div
        style={{
          height: "1px",
          width: "100%",
          background: "var(--border)",
          marginTop: "-1px",
          marginBottom: "20px",
        }}
      />

      {sections?.[tabIndex].content}
    </>
  );
};

const TabItem = ({
  active,
  title,
  onClick,
}: {
  active: boolean;
  onClick?: () => void;
  title?: string;
}) => {
  return (
    <div className={
      cn(
        "p-2",
        active && "border border-[var(--border)]",
        !active && "border border-transparent",
        "rounded-tl-lg",
        "rounded-tr-lg",
      )
    }
      style={{
        color: active ? "var(--accent-text)" : "var(--color)",
        cursor: "pointer",
        borderBottom: active ? "1px solid var(--bg-color)" : 0
      }}
      onClick={onClick}
    >
      {title}
    </div>
  );
};

export default Tabs;
