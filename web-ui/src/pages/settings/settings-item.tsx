import { ReactNode } from "@tanstack/react-router";

const SettingsItem = ({
  title,
  subTitle,
  defaultValue,
  children,
  rowOnly,
  labelFor,
  vertical
}: {
  vertical?: boolean;
  rowOnly?: boolean;
  children?: ReactNode;
  title: string;
  subTitle?: string;
  defaultValue?: string;
  labelFor?: string;
}) => {
  let classNames = ["settings-item"];
  if (rowOnly) {
    classNames.push("row-only");
  }
  if (vertical) {
    classNames = ["flex align-items-stretch flex-col gap-1"];
  }
  return (
    <div>
      <div className={classNames.join(" ")}>
        <div style={{ maxWidth: vertical ? "100%" : "400px" }}>
          <label
            htmlFor={labelFor}
            style={{ fontWeight: "bold", fontSize: ".8rem", display: "block" }}
          >
            {title}
          </label>
          <label
            htmlFor={labelFor}
            style={{ fontSize: ".8rem", opacity: 0.6, display: "block" }}
          >
            {subTitle}
            <br />
            {defaultValue && `Default: ${defaultValue}`}
          </label>
        </div>
        <div style={{ flexShrink: 0 }}>{children}</div>
      </div>
    </div>
  );
};

export default SettingsItem;
