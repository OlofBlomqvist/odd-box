import { ReactNode } from "@tanstack/react-router";

const SettingsItem = ({
  title,
  subTitle,
  defaultValue,
  children,
  rowOnly,
  labelFor,
  vertical,
  dangerText,
}: {
  vertical?: boolean;
  rowOnly?: boolean;
  children?: ReactNode;
  title: string;
  subTitle?: string;
  defaultValue?: string|number;
  labelFor?: string;
  dangerText?: ReactNode;
}) => {
  let classNames = ["settings-item"];
  if (rowOnly) {
    classNames.push("row-only");
  }
  if (vertical) {
    classNames = ["flex align-items-stretch flex-col gap-1"];
  }

  return (
    <div className={classNames.join(" ")}>
      <div style={{ maxWidth: vertical ? "100%" : "400px" }}>
        <label
          htmlFor={labelFor}
          style={{ fontWeight: "bold", fontSize: ".8rem", display: "block" }}
        >
          {title}
        </label>
        {subTitle && (
          <label
            htmlFor={labelFor}
            className="text-muted-foreground"
            style={{ fontSize: ".8rem", display: "block" }}
          >
            {subTitle}
            <br />
            {defaultValue && `Default: ${defaultValue}`}
          </label>
        )}
        <label className="text-[.8rem] text-muted-foreground block" htmlFor={labelFor}>
        {dangerText}
        </label>
      </div>
      <div style={{ flexShrink: 0 }}>{children}</div>
    </div>
  );
};

export default SettingsItem;
