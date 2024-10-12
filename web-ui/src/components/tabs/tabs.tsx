import { useRouter } from "@tanstack/react-router";
import { useState } from "react";
import { TTab } from "./types";
import { cn } from "@/lib/cn";

const Tabs = ({ sections }: { sections?: TTab[] }) => {
  const router = useRouter();
  const searchParams = new URLSearchParams(window.location.search);
  const tab = searchParams.get("tab");

  const [tabIndex, setTabIndex] = useState(Number(tab) ?? 0);

  return (
    <>
      <div style={{ display: "flex", gap: "20px" }} className="pl-[20px] md:pl-0">
        {sections?.map((section, index) => (
          <TabItem
            key={index}
            active={tabIndex === index}
            onClick={() => {
              router.navigate({ search: { tab: index } });
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
          background: "#ffffff44",
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
        active && "border border-[#ffffff44]",
        !active && "border border-transparent",
        "rounded-tl-lg",
        "rounded-tr-lg",
      )
    }
      style={{
        color: active ? "var(--color2)" : "#fff",
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
